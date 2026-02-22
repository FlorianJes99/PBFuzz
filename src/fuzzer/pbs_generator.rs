use crate::fuzzer::{
    constraint::Constraint,
    constraint_generator::{ConstraintGenerator, ConstraintSettings},
    domain::AtomIdent,
    grammar::{PBGrammar, PBKind},
};
use itertools::Itertools;
use log::info;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

use rand::{SeedableRng, rngs::StdRng};

#[derive(Debug)]
pub struct PBSGenerator {
    seed: u64,
    constraint_generator: ConstraintGenerator,
    settings: GeneratorSettings,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct GeneratorSettings {
    /// maximal amount of different variables used in the grammar
    max_vars: u32,
    /// different methods to determine when to stop generation of constraints
    cancel_condition: CancelCondition,
    /// which type of grammar we want to generate: PBS | PBO
    grammar_kind: PBKind,
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
            "Problem Task                                 : {}",
            self.grammar_kind
        )?;
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
    grammar_kind: PBKind,
    coeff_range: u32,
    amount_atoms_per_constraint: u32,
    lower_bound_degree: f64,
    upper_bound_degree: f64,
}

impl Default for GeneratorSettingsBuilder {
    fn default() -> Self {
        let cancel_condition = CancelCondition::CountConstraints {
            max_constraints: 150,
        };
        GeneratorSettingsBuilder::new(100)
            .max_unsat_for_single_constraint(3)
            .cancel_condition(cancel_condition)
            .coeff_range(200)
    }
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
        let grammar_kind = PBKind::PBS;
        // assignment combinations
        Self {
            max_vars,
            max_unsat_for_single_constraint,
            cancel_condition,
            grammar_kind,
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
            grammar_kind: settings.grammar_kind,
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

    pub fn pb_kind(mut self, grammar_kind: PBKind) -> Self {
        self.grammar_kind = grammar_kind;
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
            grammar_kind: self.grammar_kind,
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

    pub fn create_grammar(seed: u64, settings: GeneratorSettings) -> PBGrammar {
        Self::new(seed, settings).random_grammar()
    }

    fn random_grammar(&mut self) -> PBGrammar {
        let cancel_condition = self.settings.cancel_condition;
        let mut constraints: Vec<Box<dyn Constraint>> = vec![];
        let mut unsat_assignments = Option::None;
        match cancel_condition {
            CancelCondition::CountUnsat {
                max_unsat_assignments,
            } => {
                info!(
                    "Generating a maximum of {} unsat assignments",
                    max_unsat_assignments
                );
                let mut current_unsat = 0;
                while current_unsat <= max_unsat_assignments {
                    let c = self
                        .constraint_generator
                        .generate_random_constraint_with_unsat_count();
                    let unsat_for_c = c.amount_unsat_assignments();
                    current_unsat += unsat_for_c as u64;
                    constraints.push(c);
                }
                unsat_assignments = Option::Some(current_unsat);
            }
            CancelCondition::CountConstraints { max_constraints } => {
                info!("Generating {} constraints", max_constraints);
                for _ in 0..max_constraints {
                    let c = self.constraint_generator.generate_random_constraint();
                    constraints.push(c);
                }
                assert_eq!(max_constraints, constraints.len() as u64)
            }
        };

        let distinct_atom_ids: Vec<AtomIdent> = constraints
            .iter()
            .flat_map(|c| c.atoms())
            .map(|a| a.atom_id)
            .unique()
            .collect();

        let min_constraint = match self.settings.grammar_kind {
            PBKind::PBS => Option::None,
            PBKind::PBO => Some(
                self.constraint_generator
                    .generate_min_function(&distinct_atom_ids, 5),
            ),
        };

        info!("Generated {} constraints!", constraints.len());
        PBGrammar::new(
            self.seed,
            unsat_assignments,
            distinct_atom_ids.len() as u32,
            constraints,
            min_constraint,
        )
    }
}
