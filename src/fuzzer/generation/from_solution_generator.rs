use crate::fuzzer::{
    constraints::constraint::ConstraintKind,
    generation::{
        builders::{
            doubling_constraint_builder::{
                DoublingPairsSettings, create_sat_doubling_pairs_constraint,
            },
            generic_builder::{GenericFromSolutionSettings, create_sat_generic},
        },
        constraint_errors::ConstraintGenerationError,
    },
    pb_settings::FromSolutionBuildersConfig,
};
use rand::{
    distr::{Distribution, weighted::WeightedIndex},
    seq::IndexedRandom,
};

use crate::fuzzer::{
    constraints::constraint::Constraint,
    domain::{LiteralIdent, SolutionModel},
    generation::literal_sampler::LiteralSampler,
};

pub struct FromSolutionData {
    pub constraints: Vec<Box<dyn Constraint>>,
    pub solution: SolutionModel,
}

pub struct FromSolutionGenerator {
    index: WeightedIndex<u32>,
    settings: FromSolutionSettings,
}

impl FromSolutionGenerator {
    pub fn new(config: FromSolutionBuildersConfig, amount_constraints: u64) -> Self {
        let FromSolutionBuildersConfig {
            generic,
            doubling_pairs,
        } = config;
        let weights = [generic.weight, doubling_pairs.weight];
        let index = WeightedIndex::new(weights).expect("Could not create weighted index");
        let settings = FromSolutionSettings {
            amount_constraints,
            generic: generic.settings.into(),
            doubling_pairs: doubling_pairs.settings.into(),
        };
        Self { index, settings }
    }
}

struct FromSolutionSettings {
    amount_constraints: u64,
    generic: GenericFromSolutionSettings,
    doubling_pairs: DoublingPairsSettings,
}

impl FromSolutionGenerator {
    /// Generate constraints to a given assignemnts of the literals
    ///
    /// Instead of sampling random constraints, first sample a subset of literals, which will be assigned
    /// to true. Then we only generate constraints, which are satisfiable under the given solution.
    pub fn generate_from_solution(
        &self,
        literal_sampler: &mut LiteralSampler,
    ) -> Result<FromSolutionData, ConstraintGenerationError> {
        let amount_vars = literal_sampler.max_vars;
        let lits: Vec<_> = (1..=amount_vars).map(LiteralIdent::new).collect();
        let true_lits: Vec<_> = lits
            .sample(&mut literal_sampler.rand, (amount_vars / 3) as usize)
            .cloned()
            .collect();
        let solution: SolutionModel = true_lits.iter().cloned().into();
        let false_lits: Vec<_> = lits
            .iter()
            .filter(|l| !solution.contains(l))
            .cloned()
            .collect();
        let constraints = (0..self.settings.amount_constraints)
            .map(|_| {
                self.sample_sat_constraint(literal_sampler, &solution, &true_lits, &false_lits)
            })
            .collect::<Result<_, _>>()?;
        Ok(FromSolutionData {
            constraints,
            solution,
        })
    }

    fn sample_sat_constraint(
        &self,
        literal_sampler: &mut LiteralSampler,
        solution: &SolutionModel,
        true_lits: &[LiteralIdent],
        false_lits: &[LiteralIdent],
    ) -> Result<Box<dyn Constraint>, ConstraintGenerationError> {
        let constraint_kind = self.sample_constraint_kind(literal_sampler);
        match constraint_kind {
            ConstraintKind::ArbitraryLength => {
                let c = create_sat_generic(self.settings.generic, literal_sampler, solution)?;
                Ok(Box::new(c))
            }
            ConstraintKind::DoublingPairs => {
                let c = create_sat_doubling_pairs_constraint(
                    self.settings.doubling_pairs,
                    literal_sampler,
                    true_lits,
                    false_lits,
                )?;
                Ok(Box::new(c))
            }
            _ => {
                panic!("Unsupported")
            }
        }
    }

    fn sample_constraint_kind(&self, literal_sampler: &mut LiteralSampler) -> ConstraintKind {
        let index = self.index.sample(&mut literal_sampler.rand);
        match index {
            0 => ConstraintKind::ArbitraryLength,
            1 => ConstraintKind::DoublingPairs,
            _ => unreachable!(),
        }
    }
}
