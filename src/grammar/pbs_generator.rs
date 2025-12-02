use crate::grammar::domain::{Atom, AtomIdent, Comparison, Constraint, ThreeAtomConstraint};
use itertools::Itertools;
use log::info;
use std::{
    collections::HashMap,
    fmt::Display,
    i64,
    io::{self, Write},
};

use rand::{Rng, SeedableRng, rngs::StdRng, seq::index};

/// Pseudo Boolean grammar
#[derive(Debug)]
pub struct PBSGrammar {
    seed: u64,
    amount_vars: u32,
    amount_constraints: u32,
    amount_unsat_assignments: u64,
    constraints: Vec<ThreeAtomConstraint>,
}

#[derive(Clone, Copy)]
struct OccurenceWithId {
    atom_id: AtomIdent,
    count: u32,
}

impl OccurenceWithId {
    fn new(atom_id: AtomIdent, count: u32) -> Self {
        Self { atom_id, count }
    }
}

impl Display for OccurenceWithId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.count, self.atom_id)
    }
}
/// Container for various statistics about the generated grammar
pub struct GrammarStatistics {
    amount_constraints: u32,
    amount_vars: u32,
    total_var_occurences: u32,
    max_occurence: OccurenceWithId,
    min_occurence: OccurenceWithId,
    average_occurence: f32,
}

impl Display for GrammarStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "**********Statistics**********")?;
        writeln!(f, "* Amount different used variables: {}", self.amount_vars)?;
        writeln!(f, "* Amount constraints: {}", self.amount_constraints)?;
        writeln!(
            f,
            "* Total amount of used vars: {}",
            self.total_var_occurences
        )?;
        writeln!(f, "* Max occurence of a var: {}", self.max_occurence)?;
        writeln!(f, "* Min occurence of a var: {}", self.min_occurence)?;
        writeln!(
            f,
            "* Average occurences of vars: {}",
            self.average_occurence
        )?;
        writeln!(f, "******************************")
    }
}

impl PBSGrammar {
    fn new(
        seed: u64,
        amount_unsat_assignments: u64,
        constraints: Vec<ThreeAtomConstraint>,
    ) -> Self {
        let amount_constraints = constraints
            .len()
            .try_into()
            .expect("We should never generate more constraints than 2**32");
        let amount_vars = constraints
            .iter()
            .flat_map(|c| c.atoms())
            .map(|a| a.atom_id.literal_id())
            .unique()
            .count()
            .try_into()
            .expect("We should never try to create programs with more than 2**32 vars");
        Self {
            seed,
            amount_vars,
            amount_constraints,
            amount_unsat_assignments,
            constraints,
        }
    }

    pub fn produce<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        info!(
            "Generated random grammar PBS grammar with {} vars and {} constraints.",
            self.amount_vars, self.amount_constraints
        );
        let amount_equal_constraints = 0; //TODO: implement this as soon as constraints can be
        //equal with RHS
        writeln!(
            writer,
            "* #variable= {} #constraint= {} #equal= {} intsize= 32",
            self.amount_vars, self.amount_constraints, amount_equal_constraints
        )?;
        writeln!(writer, "* Used seed: {}", self.seed)?;
        writeln!(
            writer,
            "* UNSAT assignments: {}",
            self.amount_unsat_assignments
        )?;
        writeln!(writer, "{}", self.get_statistics())?;

        // write every constraint into the buffer
        for constraint in &self.constraints {
            writeln!(writer, "{}", constraint)?;
        }
        writer.flush()
    }

    pub fn get_statistics(&self) -> GrammarStatistics {
        let mut occurences_per_var: HashMap<AtomIdent, u32> = HashMap::new();
        for c in &self.constraints {
            for a in c.atoms() {
                occurences_per_var
                    .entry(a.atom_id)
                    .and_modify(|v| *v += 1)
                    .or_insert(1);
            }
        }
        let mut max: Option<OccurenceWithId> = Option::None;
        let mut min: Option<OccurenceWithId> = Option::None;
        let mut sum = 0;
        for (id, occurences) in occurences_per_var.iter() {
            if max.is_none() {
                max = Some(OccurenceWithId::new(id.clone(), *occurences));
            } else if max.is_some_and(|f| *occurences > f.count) {
                max = Some(OccurenceWithId::new(id.clone(), *occurences));
            }
            if min.is_none() {
                min = Some(OccurenceWithId::new(id.clone(), *occurences));
            } else if min.is_some_and(|f| *occurences < f.count) {
                min = Some(OccurenceWithId::new(id.clone(), *occurences));
            }
            sum += occurences;
        }
        let average = sum as f32 / occurences_per_var.len() as f32;
        GrammarStatistics {
            amount_constraints: self.amount_constraints,
            amount_vars: self.amount_vars,
            total_var_occurences: sum,
            max_occurence: max.expect("There should always be a var which occurs the most"),
            min_occurence: min.expect("There should always be a var which occurs the least"),
            average_occurence: average,
        }
    }
}

#[derive(Debug)]
pub struct PBSGenerator {
    settings: GeneratorSettings,
    rand: StdRng,
}

/// This struct holds all possible configuration parameters for the generation of a random grammar
#[derive(Debug)]
pub struct GeneratorSettings {
    /// seed used for the random generator
    seed: u64,
    /// maximal amount of different variables used in the grammar
    max_vars: u32,
    max_unsat_for_single_constraint: u32,
    /// threshold for total unsat assignments - this is an upper bound, because unsats can be
    /// duplicates
    max_unsat_assignments: u64,
    /// The possible values a coefficient of a atom can be
    /// E.g.: if `coeff_range = 8`, then values can range from -8..8 except 0
    coeff_range: u32,
    /// How many atoms a single constraint should have
    amount_atoms_per_constraint: u32,
    /// Given the sampled atoms in a constraint, the lower bound specifies the smallest possible
    /// fraction of the given range
    /// E.g.: If possible assignments yield a range of 0..10 and `lower_bound_degree = 0.3`, then
    ///       lowest possible degree (RHS) can be 3
    lower_bound_degree: f64,
    /// Given the sampled atoms in a constraint, the upper bound specifies the highest possible
    /// fraction of the given range
    /// E.g.: If possible assignments yield a range of 0..10 and `upper_bound_degree = 0.6`, then
    ///       highest possible degree (RHS) can be 6
    upper_bound_degree: f64,
}

#[derive(Debug)]
pub struct GeneratorSettingsBuilder {
    seed: u64,
    max_vars: u32,
    max_unsat_for_single_constraint: u32,
    max_unsat_assignments: u64,
    coeff_range: u32,
    amount_atoms_per_constraint: u32,
    lower_bound_degree: f64,
    upper_bound_degree: f64,
}

#[allow(dead_code)]
impl GeneratorSettingsBuilder {
    pub fn new(seed: u64, max_vars: u32) -> Self {
        let coeff_range = 3;
        let amount_atoms_per_constraint = 3;
        let max_unsat_for_single_constraint = 5;
        let lower_bound_degree = 0.3;
        let upper_bound_degree = 0.5;
        let max_unsat_assignments = (max_vars as f32 * 2.4).floor() as u64;
        // assignment combinations
        Self {
            seed,
            max_vars,
            max_unsat_for_single_constraint,
            max_unsat_assignments,
            coeff_range,
            amount_atoms_per_constraint,
            lower_bound_degree,
            upper_bound_degree,
        }
    }

    pub fn from_settings(settings: GeneratorSettings) -> Self {
        GeneratorSettingsBuilder {
            seed: settings.seed,
            max_vars: settings.max_vars,
            max_unsat_for_single_constraint: settings.max_unsat_for_single_constraint,
            max_unsat_assignments: settings.max_unsat_assignments,
            coeff_range: settings.coeff_range,
            amount_atoms_per_constraint: settings.amount_atoms_per_constraint,
            lower_bound_degree: settings.lower_bound_degree,
            upper_bound_degree: settings.upper_bound_degree,
        }
    }

    pub fn max_unsat_for_single_constraint(mut self, max: u32) -> Self {
        self.max_unsat_for_single_constraint = max;
        self
    }

    pub fn max_unsat_assignments(mut self, max: u64) -> Self {
        self.max_unsat_assignments = max;
        self
    }

    pub fn coeff_range(mut self, range: u32) -> Self {
        self.coeff_range = range;
        self
    }

    pub fn amount_atoms_per_constraint(mut self, amount: u32) -> Self {
        self.amount_atoms_per_constraint = amount;
        self
    }

    pub fn lower_bound_degree(mut self, lower: f64) -> Self {
        self.lower_bound_degree = lower;
        self
    }

    pub fn upper_bound_degree(mut self, upper: f64) -> Self {
        self.upper_bound_degree = upper;
        self
    }

    pub fn build(self) -> GeneratorSettings {
        GeneratorSettings {
            seed: self.seed,
            max_vars: self.max_vars,
            max_unsat_for_single_constraint: self.max_unsat_for_single_constraint,
            max_unsat_assignments: self.max_unsat_assignments,
            coeff_range: self.coeff_range,
            amount_atoms_per_constraint: self.amount_atoms_per_constraint,
            lower_bound_degree: self.lower_bound_degree,
            upper_bound_degree: self.upper_bound_degree,
        }
    }
}

impl PBSGenerator {
    fn new(settings: GeneratorSettings) -> Self {
        let seed = settings.seed;
        Self {
            settings,
            rand: StdRng::seed_from_u64(seed),
        }
    }

    pub fn create_grammar(settings: GeneratorSettings) -> PBSGrammar {
        Self::new(settings).random_grammar()
    }

    fn random_grammar(&mut self) -> PBSGrammar {
        let max_unsat_assignments = self.settings.max_unsat_assignments;
        info!(
            "Generating a maximum of {} unsat assignments",
            max_unsat_assignments
        );
        let mut current_unsat = 0;
        let mut constraints = vec![];
        while current_unsat < max_unsat_assignments {
            let c = self.random_constraint();
            let unsat_for_c = c.unsat_count_three_atoms() as u32;
            if unsat_for_c <= self.settings.max_unsat_for_single_constraint {
                current_unsat += unsat_for_c as u64;
                constraints.push(c);
            }
        }
        info!("Generated {} constraints!", constraints.len());
        PBSGrammar::new(self.settings.seed, current_unsat, constraints)
    }

    fn random_constraint(&mut self) -> ThreeAtomConstraint {
        // range for coefficient of each atom
        let coeff_range = self.settings.coeff_range as i32;
        let amount_atoms = self.settings.amount_atoms_per_constraint;
        // at least one atom should be in a constraint
        assert!(
            amount_atoms == 3,
            "Currently only constraints with 3 atoms are supported"
        );
        let mut ids: [usize; 3] =
            index::sample_array(&mut self.rand, self.settings.max_vars as usize)
                .expect("Invalid config for sampling. Constraint length > max_vars");
        ids.sort();
        // increment all ids by 1, because `sample_array` samples from range 0..len
        let atoms = [
            Atom::new(
                (ids[0] + 1) as u32,
                self.sample_without_zero_from_range(-coeff_range, coeff_range),
            ),
            Atom::new(
                (ids[1] + 1) as u32,
                self.sample_without_zero_from_range(-coeff_range, coeff_range),
            ),
            Atom::new(
                (ids[2] + 1) as u32,
                self.sample_without_zero_from_range(-coeff_range, coeff_range),
            ),
        ];
        let comparison = Comparison::GrEqual;
        self.create_from_atoms(atoms, comparison)
    }

    fn sample_without_zero_from_range(&mut self, start_val: i32, end_val: i32) -> i32 {
        loop {
            let rand_sample = self.rand.random_range(start_val..=end_val);
            if rand_sample != 0 {
                return rand_sample;
            }
        }
    }

    fn create_from_atoms(
        &mut self,
        atoms: [Atom; 3],
        comparison: Comparison,
    ) -> ThreeAtomConstraint {
        let mut min_coeff = i64::MAX;
        let mut max_coeff = i64::MIN;
        let mut min_sum_coeff = 0;
        for coeff in atoms.iter().map(|a| a.coeff as i64) {
            if coeff < 0 {
                min_sum_coeff += coeff;
            }
            if coeff > max_coeff {
                max_coeff = coeff;
            }
            if coeff < min_coeff {
                min_coeff = coeff
            }
        }
        let distance = (max_coeff - min_coeff).abs() as f64;
        let range_start =
            min_sum_coeff + (self.settings.lower_bound_degree * distance).floor() as i64;
        let range_end = min_coeff + (self.settings.upper_bound_degree * distance).ceil() as i64;
        let range_start = min_coeff.max(range_start);
        // this ensures that we have always a single atom, which can already fulfill the equation
        let range_end = max_coeff.min(range_end);
        assert!(
            range_start <= range_end,
            "Range start should be smaller than / equal to range end"
        );
        ThreeAtomConstraint::new(
            atoms,
            comparison,
            self.rand.random_range(range_start..=range_end),
        )
    }
}

#[test]
fn test_statistics() {
    let atoms1 = [Atom::new(1, 2), Atom::new(2, 5), Atom::new(4, 1)];
    let atoms2 = [Atom::new(2, 2), Atom::new(3, 5), Atom::new(4, 1)];
    let atoms3 = [Atom::new(1, 2), Atom::new(3, 5), Atom::new(4, 1)];
    let constraints = vec![
        ThreeAtomConstraint::new(atoms1, Comparison::GrEqual, 5),
        ThreeAtomConstraint::new(atoms2, Comparison::GrEqual, 5),
        ThreeAtomConstraint::new(atoms3, Comparison::GrEqual, 5),
    ];
    let unsat_assignments = constraints
        .iter()
        .map(|c| c.unsat_count_three_atoms() as u64)
        .sum();
    let grammar = PBSGrammar::new(42, unsat_assignments, constraints);
    let stats = grammar.get_statistics();
    assert_eq!(stats.amount_vars, 4);
    assert_eq!(stats.amount_constraints, 3);
    assert_eq!(stats.max_occurence.count, 3);
    assert_eq!(stats.min_occurence.count, 2);
    assert_eq!(stats.total_var_occurences, 9);
    assert!((stats.average_occurence - 2.25).abs() < 0.001);
}
