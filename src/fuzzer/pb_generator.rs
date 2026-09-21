use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;
use crate::fuzzer::generation::from_solution_generator::FromSolutionGenerator;
use crate::fuzzer::pb_settings::GenerationMethodWithSettings;
use crate::fuzzer::{
    app_info::MetaData,
    constraints::constraint::Constraint,
    domain::{LiteralIdent, MinFunction},
    generation::{
        from_solution_generator::FromSolutionData, literal_sampler::LiteralSampler,
        random_constraints_generator::RandomConstraintGenerator,
    },
    grammar::{GenerationSource, PBGrammar, PBKind},
    pb_settings::{CancelCondition, PbSetting},
};
use itertools::Itertools;
use log::info;
use std::time::Instant;
use thiserror::Error;

use rand::{SeedableRng, rngs::StdRng, seq::IndexedRandom};

pub struct PBGenerator {
    seed: u64,
    literal_sampler: LiteralSampler,
    settings: PbSetting,
}

impl PBGenerator {
    fn new(seed: u64, settings: PbSetting) -> Self {
        let literal_sampler =
            LiteralSampler::new(StdRng::seed_from_u64(seed), settings.get_max_vars());
        Self {
            seed,
            literal_sampler,
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
                    PBKind::PBO => Some(self.generate_min_function(&distinct_literal_ids, 5)),
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
                match self.settings.get_generator_settings() {
                    GenerationMethodWithSettings::FromSolution(generator_settings) => {
                        let generator = FromSolutionGenerator::new(
                            *generator_settings.get_builder_config(),
                            *max_constraints,
                        );
                        let FromSolutionData {
                            constraints,
                            solution,
                        } = generator.generate_from_solution(&mut self.literal_sampler)?;
                        let distinct_literal_ids: Vec<LiteralIdent> =
                            distinct_literal_ids(&constraints);
                        let min_constraint = match self.settings.get_problem_kind() {
                            PBKind::PBS => Option::None,
                            PBKind::PBO => {
                                Some(self.generate_min_function(&distinct_literal_ids, 20))
                            }
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

                    GenerationMethodWithSettings::Random(generator_settings) => {
                        let builder_config = generator_settings.get_builder_config();
                        let random_generator = RandomConstraintGenerator::new(*builder_config);
                        let constraints: Vec<_> = (0..*max_constraints)
                            .map(|_| {
                                random_generator
                                    .generate_random_constraint(&mut self.literal_sampler)
                            })
                            .collect::<Result<_, _>>()?;
                        assert_eq!(*max_constraints, constraints.len() as u64);

                        let distinct_literal_ids: Vec<LiteralIdent> =
                            distinct_literal_ids(&constraints);
                        let min_constraint = match self.settings.get_problem_kind() {
                            PBKind::PBS => Option::None,
                            PBKind::PBO => {
                                Some(self.generate_min_function(&distinct_literal_ids, 5))
                            }
                        };
                        Ok(PBGrammar::new(
                            program_info,
                            self.seed,
                            distinct_literal_ids.len() as u32,
                            constraints,
                            min_constraint,
                            start.elapsed(),
                            GenerationSource::Random {
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
        let generator_settings = match self.settings.get_generator_settings() {
            GenerationMethodWithSettings::Random(generator_settings) => generator_settings,
            GenerationMethodWithSettings::FromSolution(_) => panic!("Not supported"),
        };
        let random_generator =
            RandomConstraintGenerator::new(*generator_settings.get_builder_config());
        let mut constraints: Vec<Box<dyn Constraint>> = vec![];
        let mut current_unsat = 0;
        while current_unsat <= max_unsat_assignments {
            let c = random_generator
                .generate_random_constraint_with_unsat_count(&mut self.literal_sampler)?;
            let unsat_for_c = c.amount_unsat_assignments();
            current_unsat += unsat_for_c as u64;
            constraints.push(c);
        }
        Ok((constraints, current_unsat))
    }

    fn generate_min_function(
        &mut self,
        possible_vars: &[LiteralIdent],
        amount: usize,
    ) -> MinFunction {
        let literals = possible_vars
            .sample(&mut self.literal_sampler.rand, amount)
            .sorted()
            .map(|id| id.to_literal(self.literal_sampler.sample_without_zero_from_range(-3, 3)))
            .collect();
        MinFunction::new(literals)
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
