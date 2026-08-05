use crate::fuzzer::{
    app_info::MetaData,
    constraint::Constraint,
    constraint_generator::{ConstraintGenerationError, ConstraintGenerator, FromSolutionData},
    domain::LiteralIdent,
    grammar::{GenerationSource, PBGrammar, PBKind},
    pb_settings::{CancelCondition, GenerationMethod, PbSetting},
};
use itertools::Itertools;
use log::info;
use std::time::Instant;
use thiserror::Error;

use rand::{rngs::StdRng, SeedableRng};

#[derive(Debug)]
pub struct PBGenerator {
    seed: u64,
    constraint_generator: ConstraintGenerator,
    settings: PbSetting,
}

impl PBGenerator {
    fn new(seed: u64, settings: PbSetting) -> Self {
        let constr_generator = ConstraintGenerator::new(
            StdRng::seed_from_u64(seed),
            settings.get_generator_settings().into(),
        );
        Self {
            seed,
            constraint_generator: constr_generator,
            settings,
        }
    }

    pub fn create_grammar(
        seed: u64,
        settings: PbSetting,
        program_info: MetaData,
    ) -> Result<PBGrammar, PbGenerationError> {
        Self::new(seed, settings)
            .random_grammar(program_info)
            .map_err(|e| PbGenerationError {
                seed,
                error_type: e,
            })
    }

    fn random_grammar(
        &mut self,
        program_info: super::app_info::MetaData,
    ) -> Result<PBGrammar, ConstraintGenerationError> {
        let start = Instant::now();
        match self
            .settings
            .get_generator_settings()
            .get_cancel_condition()
        {
            CancelCondition::CountUnsat {
                max_unsat_assignments,
            } => {
                info!(
                    "Generating a maximum of {} unsat assignments",
                    max_unsat_assignments
                );
                let (constraints, current_unsat) =
                    self.generate_until_max_unsat(*max_unsat_assignments)?;
                let distinct_literal_ids: Vec<LiteralIdent> = distinct_literal_ids(&constraints);
                let min_constraint = match self.settings.get_problem_kind() {
                    PBKind::PBS => Option::None,
                    PBKind::PBO => Some(
                        self.constraint_generator
                            .generate_min_function(&distinct_literal_ids, 5),
                    ),
                };
                Ok(PBGrammar::new(
                    program_info,
                    self.seed,
                    distinct_literal_ids.len() as u32,
                    constraints,
                    min_constraint,
                    start.elapsed(),
                    crate::fuzzer::grammar::GenerationSource::Random {
                        amount_unsat_assignments: Option::Some(current_unsat),
                    },
                ))
            }
            CancelCondition::CountConstraints { max_constraints } => {
                info!("Generating {} constraints", max_constraints);
                match self.settings.get_generation_method() {
                    GenerationMethod::FromSolution => {
                        let FromSolutionData {
                            constraints,
                            solution,
                        } = self
                            .constraint_generator
                            .generate_from_solution(*max_constraints)?;
                        let distinct_literal_ids: Vec<LiteralIdent> =
                            distinct_literal_ids(&constraints);
                        let min_constraint = match self.settings.get_problem_kind() {
                            PBKind::PBS => Option::None,
                            PBKind::PBO => Some(
                                self.constraint_generator
                                    .generate_min_function(&distinct_literal_ids, 20),
                            ),
                        };
                        let objective = min_constraint.as_ref().map(|obj| {
                            obj.get_literals()
                                .iter()
                                .filter(|l| solution.contains(&l.literal_id))
                                .map(|l| l.coeff)
                                .sum::<i32>()
                                .into()
                        });
                        Ok(PBGrammar::new(
                            program_info,
                            self.seed,
                            distinct_literal_ids.len() as u32,
                            constraints,
                            min_constraint,
                            start.elapsed(),
                            GenerationSource::FromSolution {
                                model: solution,
                                objective,
                            },
                        ))
                    }

                    GenerationMethod::Random => {
                        let constraints: Vec<_> = (0..*max_constraints)
                            .map(|_| self.constraint_generator.generate_random_constraint())
                            .collect::<Result<_, _>>()?;
                        assert_eq!(*max_constraints, constraints.len() as u64);

                        let distinct_literal_ids: Vec<LiteralIdent> =
                            distinct_literal_ids(&constraints);
                        let min_constraint = match self.settings.get_problem_kind() {
                            PBKind::PBS => Option::None,
                            PBKind::PBO => Some(
                                self.constraint_generator
                                    .generate_min_function(&distinct_literal_ids, 5),
                            ),
                        };
                        Ok(PBGrammar::new(
                            program_info,
                            self.seed,
                            distinct_literal_ids.len() as u32,
                            constraints,
                            min_constraint,
                            start.elapsed(),
                            crate::fuzzer::grammar::GenerationSource::Random {
                                amount_unsat_assignments: Option::None,
                            },
                        ))
                    }
                }
            }
        }
    }

    fn generate_until_max_unsat(
        &mut self,
        max_unsat_assignments: u64,
    ) -> Result<(Vec<Box<dyn Constraint>>, u64), ConstraintGenerationError> {
        let mut constraints: Vec<Box<dyn Constraint>> = vec![];
        let mut current_unsat = 0;
        while current_unsat <= max_unsat_assignments {
            let c = self
                .constraint_generator
                .generate_random_constraint_with_unsat_count()?;
            let unsat_for_c = c.amount_unsat_assignments();
            current_unsat += unsat_for_c as u64;
            constraints.push(c);
        }
        Ok((constraints, current_unsat))
    }
}

fn distinct_literal_ids(constraints: &[Box<dyn Constraint>]) -> Vec<LiteralIdent> {
    constraints
        .iter()
        .flat_map(|c| c.literals())
        .map(|a| a.literal_id)
        .unique()
        .collect()
}

#[derive(Debug, Error)]
#[error("Error while generating pb problem with seed {seed}: {error_type}")]
pub struct PbGenerationError {
    seed: u64,
    #[source]
    error_type: ConstraintGenerationError,
}
