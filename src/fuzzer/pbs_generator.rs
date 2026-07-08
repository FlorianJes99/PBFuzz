use crate::fuzzer::{
    constraint::Constraint,
    constraint_generator::{ConstraintGenerationError, ConstraintGenerator},
    domain::LiteralIdent,
    grammar::{PBGrammar, PBKind},
    pb_settings::{CancelCondition, GenerationMethod, GeneratorSettings},
};
use itertools::Itertools;
use log::info;
use std::time::Instant;

use rand::{rngs::StdRng, SeedableRng};

#[derive(Debug)]
pub struct PBSGenerator {
    seed: u64,
    constraint_generator: ConstraintGenerator,
    settings: GeneratorSettings,
}

impl PBSGenerator {
    fn new(seed: u64, settings: GeneratorSettings) -> Self {
        let constr_generator =
            ConstraintGenerator::new(StdRng::seed_from_u64(seed), settings.into());
        Self {
            seed,
            constraint_generator: constr_generator,
            settings,
        }
    }

    pub fn create_grammar(
        seed: u64,
        settings: GeneratorSettings,
    ) -> Result<PBGrammar, ConstraintGenerationError> {
        Self::new(seed, settings).random_grammar()
    }

    fn random_grammar(&mut self) -> Result<PBGrammar, ConstraintGenerationError> {
        let start = Instant::now();
        let cancel_condition = self.settings.get_cancel_condition();
        let (constraints, unsat_assignments) = match cancel_condition {
            CancelCondition::CountUnsat {
                max_unsat_assignments,
            } => {
                info!(
                    "Generating a maximum of {} unsat assignments",
                    max_unsat_assignments
                );
                let mut constraints_temp: Vec<Box<dyn Constraint>> = vec![];
                let mut current_unsat = 0;
                while &current_unsat <= max_unsat_assignments {
                    let c = self
                        .constraint_generator
                        .generate_random_constraint_with_unsat_count()?;
                    let unsat_for_c = c.amount_unsat_assignments();
                    current_unsat += unsat_for_c as u64;
                    constraints_temp.push(c);
                }
                (constraints_temp, Option::Some(current_unsat))
            }
            CancelCondition::CountConstraints { max_constraints } => {
                match self.settings.get_generation_method() {
                    GenerationMethod::FromSolution => (
                        self.constraint_generator
                            .generate_from_solution(*max_constraints)?,
                        Option::None,
                    ),

                    GenerationMethod::Random => {
                        info!("Generating {} constraints", max_constraints);
                        let cs: Vec<_> = (0..*max_constraints)
                            .map(|_| self.constraint_generator.generate_random_constraint())
                            .collect::<Result<_, _>>()?;
                        assert_eq!(*max_constraints, cs.len() as u64);
                        (cs, Option::None)
                    }
                }
            }
        };

        let distinct_literal_ids: Vec<LiteralIdent> = constraints
            .iter()
            .flat_map(|c| c.literals())
            .map(|a| a.literal_id)
            .unique()
            .collect();

        let min_constraint = match self.settings.get_problem_kind() {
            PBKind::PBS => Option::None,
            PBKind::PBO => Some(
                self.constraint_generator
                    .generate_min_function(&distinct_literal_ids, 5),
            ),
        };

        Ok(PBGrammar::new(
            self.seed,
            unsat_assignments,
            distinct_literal_ids.len() as u32,
            constraints,
            min_constraint,
            start.elapsed(),
        ))
    }
}
