use crate::grammar::{
    constraint::{Constraint, ConstraintKind},
    constraint_generator::{ConstraintGenerator, ConstraintSettings},
    domain::AtomIdent,
};
use itertools::Itertools;
use log::info;
use serde::Serialize;
use std::{
    collections::HashMap,
    fmt::Display,
    io::{self, Write},
};

use rand::{SeedableRng, rngs::StdRng};

/// Pseudo Boolean grammar
pub struct PBSGrammar {
    seed: u64,
    amount_vars: u32,
    amount_constraints: u32,
    amount_unsat_assignments: u64,
    constraints: Vec<Box<dyn Constraint>>,
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
    amount_constraints_by_kind: HashMap<ConstraintKind, usize>,
    amount_vars: u32,
    total_var_occurences: u32,
    max_occurence: OccurenceWithId,
    min_occurence: OccurenceWithId,
    average_occurence: f32,
}

impl Display for GrammarStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "* Amount different used variables: {}", self.amount_vars)?;
        writeln!(f, "* Amount constraints: {}", self.amount_constraints)?;
        for (k, v) in &self.amount_constraints_by_kind {
            writeln!(f, "* Amount constraints of type {k}: {v}")?;
        }
        writeln!(
            f,
            "* Total amount of used vars: {}",
            self.total_var_occurences
        )?;
        writeln!(f, "* Max occurence of a var: {}", self.max_occurence)?;
        writeln!(f, "* Min occurence of a var: {}", self.min_occurence)?;
        write!(
            f,
            "* Average occurences of vars: {}",
            self.average_occurence
        )
    }
}

impl PBSGrammar {
    fn new(
        seed: u64,
        amount_unsat_assignments: u64,
        amount_distinct_vars: u32,
        constraints: Vec<Box<dyn Constraint>>,
    ) -> Self {
        let amount_constraints = constraints
            .len()
            .try_into()
            .expect("We should never generate more constraints than 2**32");
        Self {
            seed,
            amount_vars: amount_distinct_vars,
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
        let amount_constraints_by_kind = self.constraints.iter().counts_by(|v| v.kind());
        GrammarStatistics {
            amount_constraints: self.amount_constraints,
            amount_constraints_by_kind,
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
    seed: u64,
    constraint_generator: ConstraintGenerator,
    settings: GeneratorSettings,
}

#[derive(Debug, Copy, Clone, Serialize)]
pub enum CancelCondition {
    CountUnsat { max_unsat_assignments: u64 },
    CountConstraints { max_constraints: u64 },
}

impl Display for CancelCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CancelCondition::CountUnsat {
                max_unsat_assignments,
            } => write!(
                f,
                "Max unsat assignments                        : {}",
                max_unsat_assignments
            ),
            CancelCondition::CountConstraints { max_constraints } => write!(
                f,
                "Max constraints:                             : {}",
                max_constraints
            ),
        }
    }
}

/// This struct holds all possible configuration parameters for the generation of a random grammar
#[derive(Debug, Copy, Clone, Serialize)]
pub struct GeneratorSettings {
    /// maximal amount of different variables used in the grammar
    max_vars: u32,
    /// threshold for total unsat assignments - this is an upper bound, because unsats can be
    /// duplicates
    cancel_condition: CancelCondition,
    /// How many atoms a single constraint should have
    amount_atoms_per_constraint: u32,
    max_unsat_for_single_constraint: u32,
    /// The possible values a coefficient of a atom can be
    /// E.g.: if `coeff_range = 8`, then values can range from -8..8 except 0
    coeff_range: u32,
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

impl Display for GeneratorSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Used Settings for Generator:")?;
        writeln!(
            f,
            "Max used vars                                : {}",
            self.max_vars
        )?;
        writeln!(f, "{}", self.cancel_condition)?;
        writeln!(
            f,
            "Atoms per constraint                         : {}",
            self.amount_atoms_per_constraint
        )?;
        writeln!(
            f,
            "Max unsat assignments for a single constraint: {}",
            self.max_unsat_for_single_constraint
        )?;
        writeln!(
            f,
            "Coefficient range                            : -{}..{}",
            self.coeff_range, self.coeff_range
        )?;
        writeln!(
            f,
            "Lower bound for RHS of constraint            : {}",
            self.lower_bound_degree
        )?;
        write!(
            f,
            "Upper bound for RHS of constraint            : {}",
            self.upper_bound_degree
        )
    }
}

/// Builder to create Settings for the fuzzer
///
/// Defaults:
/// `coeff_range: 3`
/// `amount_atoms_per_constraint: 3`
/// `max_unsat_for_single_constraint: 5`
/// `lower_bound_degree: 0.3`
/// `upper_bound_degree: 0.5`
/// max_unsat_assignments: max_vars * 2.4
#[derive(Debug)]
pub struct GeneratorSettingsBuilder {
    max_vars: u32,
    max_unsat_for_single_constraint: u32,
    cancel_condition: CancelCondition,
    coeff_range: u32,
    amount_atoms_per_constraint: u32,
    lower_bound_degree: f64,
    upper_bound_degree: f64,
}

impl GeneratorSettingsBuilder {
    pub fn new(max_vars: u32) -> Self {
        let coeff_range = 3;
        let amount_atoms_per_constraint = 50;
        let max_unsat_for_single_constraint = 5;
        let lower_bound_degree = 0.3;
        let upper_bound_degree = 0.5;
        let cancel_condition = CancelCondition::CountUnsat {
            max_unsat_assignments: (max_vars as f32 * 2.4).floor() as u64,
        };
        // assignment combinations
        Self {
            max_vars,
            max_unsat_for_single_constraint,
            cancel_condition,
            coeff_range,
            amount_atoms_per_constraint,
            lower_bound_degree,
            upper_bound_degree,
        }
    }

    pub fn from_settings(settings: GeneratorSettings) -> Self {
        GeneratorSettingsBuilder {
            max_vars: settings.max_vars,
            max_unsat_for_single_constraint: settings.max_unsat_for_single_constraint,
            cancel_condition: settings.cancel_condition,
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

    pub fn cancel_condition(mut self, cancel_condition: CancelCondition) -> Self {
        self.cancel_condition = cancel_condition;
        self
    }

    pub fn coeff_range(mut self, range: u32) -> Self {
        self.coeff_range = range;
        self
    }

    /// this method panics if lower > upper!
    pub fn set_bounds(mut self, lower: f64, upper: f64) -> Self {
        assert!(lower <= upper);
        self.lower_bound_degree = lower;
        self.upper_bound_degree = upper;
        self
    }

    pub fn build(self) -> GeneratorSettings {
        GeneratorSettings {
            max_vars: self.max_vars,
            max_unsat_for_single_constraint: self.max_unsat_for_single_constraint,
            cancel_condition: self.cancel_condition,
            coeff_range: self.coeff_range,
            amount_atoms_per_constraint: self.amount_atoms_per_constraint,
            lower_bound_degree: self.lower_bound_degree,
            upper_bound_degree: self.upper_bound_degree,
        }
    }
}

impl PBSGenerator {
    fn new(seed: u64, settings: GeneratorSettings) -> Self {
        let constr_generator = ConstraintGenerator::new(
            StdRng::seed_from_u64(seed),
            ConstraintSettings::new(
                settings.max_vars,
                settings.max_unsat_for_single_constraint,
                settings.coeff_range as i32,
                settings.lower_bound_degree,
                settings.upper_bound_degree,
            ),
        );
        Self {
            seed,
            constraint_generator: constr_generator,
            settings,
        }
    }

    pub fn create_grammar(seed: u64, settings: GeneratorSettings) -> PBSGrammar {
        Self::new(seed, settings).random_grammar()
    }

    fn random_grammar(&mut self) -> PBSGrammar {
        let cancel_condition = self.settings.cancel_condition;
        match cancel_condition {
            CancelCondition::CountUnsat {
                max_unsat_assignments,
            } => info!(
                "Generating a maximum of {} unsat assignments",
                max_unsat_assignments
            ),
            CancelCondition::CountConstraints { max_constraints } => {
                info!("Generating {} constraints", max_constraints)
            }
        };
        let mut current_unsat = 0;
        let mut constraints: Vec<Box<dyn Constraint>> = vec![];
        loop {
            match cancel_condition {
                CancelCondition::CountUnsat {
                    max_unsat_assignments,
                } if current_unsat >= max_unsat_assignments => break,
                CancelCondition::CountConstraints { max_constraints }
                    if constraints.len() as u64 >= max_constraints =>
                {
                    break;
                }
                _ => {
                    let c = self.constraint_generator.generate_random();
                    let unsat_for_c = c.amount_unsat_assignments();
                    current_unsat += unsat_for_c as u64;
                    constraints.push(c);
                }
            }
        }

        let amount_distinct_vars = constraints
            .iter()
            .flat_map(|c| c.atoms())
            .map(|a| a.atom_id.literal_id())
            .unique()
            .count()
            .try_into()
            .expect("We should never try to create programs with more than 2**32 vars");

        info!("Generated {} constraints!", constraints.len());
        PBSGrammar::new(self.seed, current_unsat, amount_distinct_vars, constraints)
    }
}

#[test]
fn test_statistics() {
    use crate::grammar::constraint::CostraintWithUnsatAssignments;
    use crate::grammar::constraint::ThreeAtomConstraint;
    use crate::grammar::domain::{Atom, Comparison};
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
        .map(|c| c.amount_unsat_assignments() as u64)
        .sum();
    let constraints: Vec<Box<dyn Constraint>> = constraints
        .into_iter()
        .map(|a| Box::new(a) as Box<dyn Constraint>)
        .collect();
    let grammar = PBSGrammar::new(42, unsat_assignments, 4, constraints);
    let stats = grammar.get_statistics();
    assert_eq!(stats.amount_vars, 4);
    assert_eq!(stats.amount_constraints, 3);
    assert_eq!(stats.max_occurence.count, 3);
    assert_eq!(stats.min_occurence.count, 2);
    assert_eq!(stats.total_var_occurences, 9);
    assert!((stats.average_occurence - 2.25).abs() < 0.001);
}
