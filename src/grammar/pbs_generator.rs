use crate::grammar::domain::{Atom, AtomId, Comparison, Constraint};
use itertools::Itertools;
use log::info;
use std::{
    collections::HashMap,
    fmt::Display,
    io::{self, Write},
};

use rand::{Rng, SeedableRng, rngs::StdRng, seq::index};

/// Pseudo Boolean grammar
#[derive(Debug)]
pub struct PBSGrammar {
    seed: u64,
    amount_vars: u32,
    amount_constraints: u32,
    constraints: Vec<Constraint>,
}

#[derive(Clone, Copy)]
struct OccurenceWithId {
    atom_id: AtomId,
    count: u32,
}

impl OccurenceWithId {
    fn new(atom_id: AtomId, count: u32) -> Self {
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
    fn new(seed: u64, constraints: Vec<Constraint>) -> Self {
        let amount_constraints = constraints
            .len()
            .try_into()
            .expect("We should never generate more constraints than 2**32");
        let amount_vars = constraints
            .iter()
            .flat_map(|c| c.atoms())
            .map(|a| a.atom_id.literal_id)
            .unique()
            .count()
            .try_into()
            .expect("We should never try to create programs with more than 2**32 vars");
        Self {
            seed,
            amount_vars,
            amount_constraints,
            constraints,
        }
    }

    fn amount_unsat_assignments(&self) -> u32 {
        self.constraints
            .iter()
            .map(|c| c.unsat_count_three_atoms())
            .sum()
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
            self.amount_unsat_assignments()
        )?;
        writeln!(writer, "{}", self.get_statistics())?;

        // write every constraint into the buffer
        for constraint in &self.constraints {
            writeln!(writer, "{}", constraint)?;
        }
        writer.flush()
    }

    pub fn get_statistics(&self) -> GrammarStatistics {
        let mut occurences_per_var: HashMap<AtomId, u32> = HashMap::new();
        for c in &self.constraints {
            for a in c.atoms() {
                occurences_per_var
                    .entry(a.atom_id.clone())
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
            max_occurence: max.expect("There should always a var which occurs the most"),
            min_occurence: min.expect("There should always a var which occurs the least"),
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
struct GeneratorSettings {
    /// seed used for the random generator
    seed: u64,
    /// maximal amount of different variables used in the grammar
    max_vars: u32,
    /// how many constraints should be generated
    amount_constraints: u32,
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

impl GeneratorSettings {
    fn new(seed: u64, max_vars: u32) -> Self {
        let coeff_range = 5;
        let amount_atoms_per_constraint = 3;
        let lower_bound_degree = 0.2;
        let upper_bound_degree = 0.4;
        let amount_constraints = (max_vars as f32 * 1.4).floor() as u32;
        Self {
            seed,
            max_vars,
            amount_constraints,
            coeff_range,
            amount_atoms_per_constraint,
            lower_bound_degree,
            upper_bound_degree,
        }
    }
}

impl PBSGenerator {
    fn new(seed: u64, max_vars: u32) -> Self {
        let settings = GeneratorSettings::new(seed, max_vars);
        Self {
            settings,
            rand: StdRng::seed_from_u64(seed),
        }
    }

    pub fn create_grammar(seed: u64, max_vars: u32) -> PBSGrammar {
        Self::new(seed, max_vars).random_grammar()
    }

    fn random_grammar(&mut self) -> PBSGrammar {
        let amount_constraints = self.settings.amount_constraints;
        info!("Generating amount of constraints: {}", amount_constraints);
        let constraints: Vec<Constraint> = (0..amount_constraints)
            .map(|_| self.random_constraint())
            .collect();
        info!("Generated {} constraints!", amount_constraints);
        PBSGrammar::new(self.settings.seed, constraints)
    }

    fn random_constraint(&mut self) -> Constraint {
        // range for coefficient of each atom
        let coeff_range = self.settings.coeff_range as i32;
        // generate always 3 atoms if possible
        let amount_atoms = self.settings.amount_atoms_per_constraint;
        // at least one atom should be in a constraint
        let sample = index::sample(
            &mut self.rand,
            self.settings.max_vars as usize,
            amount_atoms as usize,
        );
        let atoms: Vec<Atom> = sample
            .iter()
            .sorted()
            // add indices + 1, because sample method samples from range (0..length)
            .map(|n| n + 1)
            .map(|n| {
                Atom::new(
                    n.try_into()
                        .expect("We should always be able to cast to u32 for atom var ids"),
                    self.sample_without_zero_from_range(-coeff_range, coeff_range),
                )
            })
            .collect();

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

    fn create_from_atoms(&mut self, atoms: Vec<Atom>, comparison: Comparison) -> Constraint {
        let min_coeff: i64 = atoms
            .iter()
            .filter(|atom| atom.coeff < 0)
            .map(|atom| atom.coeff as i64)
            .sum();
        let max_coeff: i64 = atoms
            .iter()
            .filter(|atom| atom.coeff > 0)
            .map(|atom| atom.coeff as i64)
            .sum();
        let distance = (max_coeff - min_coeff).abs() as f64;
        // place the possible RHS values inside the central 20% of possible ranges
        let range_start = min_coeff + (self.settings.lower_bound_degree * distance).floor() as i64;
        let range_end = min_coeff + (self.settings.upper_bound_degree * distance).ceil() as i64;
        assert!(
            range_start <= range_end,
            "Range start should be smaller than / equal to range end"
        );
        let constraint = Constraint::new(
            atoms,
            comparison,
            self.rand.random_range(range_start..=range_end),
        );
        constraint
    }
}

#[test]
fn test_statistics() {
    let atoms1 = vec![Atom::new(1, 2), Atom::new(2, 5), Atom::new(4, 1)];
    let atoms2 = vec![Atom::new(2, 2), Atom::new(3, 5), Atom::new(4, 1)];
    let atoms3 = vec![Atom::new(1, 2), Atom::new(3, 5), Atom::new(4, 1)];
    let constraints = vec![
        Constraint::new(atoms1, Comparison::GrEqual, 5),
        Constraint::new(atoms2, Comparison::GrEqual, 5),
        Constraint::new(atoms3, Comparison::GrEqual, 5),
    ];
    let grammar = PBSGrammar::new(42, constraints);
    let stats = grammar.get_statistics();
    assert_eq!(stats.amount_vars, 4);
    assert_eq!(stats.amount_constraints, 3);
    assert_eq!(stats.max_occurence.count, 3);
    assert_eq!(stats.min_occurence.count, 2);
    assert_eq!(stats.total_var_occurences, 9);
    assert!((stats.average_occurence - 2.25).abs() < 0.001);
}
