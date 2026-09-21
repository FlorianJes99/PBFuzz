use crate::fuzzer::{
    generation::{
        builders::{
            cardinality_builder::CardinalitySettings,
            doubling_constraint_builder::DoublingPairsSettings,
            exactly_one_builder::ExactlyOneSettings, generic_builder::GenericSettings,
            three_literal_builder::ThreeLiteralSettings,
        },
        constraint_errors::ConstraintGenerationError,
    },
    pb_settings::RandomBuildersConfig,
};
use log::debug;
use rand::{
    RngExt,
    distr::{Distribution, weighted::WeightedIndex},
};

use crate::fuzzer::{
    constraints::constraint::{Constraint, ConstraintKind, CostraintWithUnsatAssignments},
    generation::{
        builders::{
            cardinality_builder::generate_cardinality,
            doubling_constraint_builder::generate_doubling_pairs_constraint,
            exactly_one_builder::generate_exactly_one_constraint,
            generic_builder::generate_generic, three_literal_builder::generate_three_literal,
        },
        literal_sampler::LiteralSampler,
    },
};

pub struct RandomConstraintGenerator {
    settings: RandomConstraintSettings,
    dist: WeightedIndex<u32>,
}

struct RandomConstraintSettings {
    three_literal: ThreeLiteralSettings,
    cardinality: CardinalitySettings,
    generic: GenericSettings,
    exactly_one: ExactlyOneSettings,
    doubling_pairs: DoublingPairsSettings,
}

impl RandomConstraintGenerator {
    pub fn new(config: RandomBuildersConfig) -> Self {
        let RandomBuildersConfig {
            three_literal,
            cardinality,
            generic,
            exactly_one,
            doubling_pairs,
        } = config;
        let weights = [
            three_literal.weight,
            cardinality.weight,
            generic.weight,
            exactly_one.weight,
            doubling_pairs.weight,
        ];
        let dist = WeightedIndex::new(weights)
            .expect("We should only create valid distributions, when sampling constraint types");
        let settings = RandomConstraintSettings {
            three_literal: three_literal.settings.into(),
            cardinality: cardinality.settings.into(),
            generic: generic.settings.into(),
            exactly_one: exactly_one.settings.into(),
            doubling_pairs: doubling_pairs.settings.into(),
        };
        Self { settings, dist }
    }

    pub fn generate_random_constraint_with_unsat_count(
        &self,
        literal_sampler: &mut LiteralSampler,
    ) -> Result<Box<dyn CostraintWithUnsatAssignments>, ConstraintGenerationError> {
        if literal_sampler.rand.random_bool(0.5) {
            let settings = self.settings.three_literal;
            let mut c = generate_three_literal(settings, literal_sampler)?;
            // Assert we always return a constraint which fulfills max unsat assignments requirement
            let unsat_single_constraint: u128 = 3;
            while c.amount_unsat_assignments() > unsat_single_constraint {
                c = generate_three_literal(settings, literal_sampler)?;
            }
            Ok(Box::new(c))
        } else {
            let settings = self.settings.cardinality;
            let c = generate_cardinality(settings, literal_sampler)?;
            Ok(Box::new(c))
        }
    }

    pub fn generate_random_constraint(
        &self,
        literal_sampler: &mut LiteralSampler,
    ) -> Result<Box<dyn Constraint>, ConstraintGenerationError> {
        let kind = self.sample_constraint_kind(literal_sampler);
        match kind {
            ConstraintKind::ThreeLiteral => {
                let settings = self.settings.three_literal;
                let mut c = generate_three_literal(settings, literal_sampler)?;
                // Assert we always return a constraint which fulfills max unsat assignments requirement
                while c.amount_unsat_assignments() > 3 {
                    debug!("Looping for valid three literal constraint");
                    c = generate_three_literal(settings, literal_sampler)?;
                }
                Ok(Box::new(c))
            }
            ConstraintKind::Cardinality => {
                let settings = self.settings.cardinality;
                let c = generate_cardinality(settings, literal_sampler)?;
                Ok(Box::new(c))
            }
            ConstraintKind::ArbitraryLength => {
                let settings = self.settings.generic;
                let c = generate_generic(settings, literal_sampler)?;
                Ok(Box::new(c))
            }
            ConstraintKind::ExactlyOne => {
                // we want to have at max 3 literals in the exactly one constraint for now
                let settings = self.settings.exactly_one;
                let c = generate_exactly_one_constraint(settings, literal_sampler)?;
                Ok(Box::new(c))
            }
            ConstraintKind::DoublingPairs => {
                let settings = self.settings.doubling_pairs;
                let c = generate_doubling_pairs_constraint(settings, literal_sampler)?;
                Ok(Box::new(c))
            }
        }
    }

    fn sample_constraint_kind(&self, literal_sampler: &mut LiteralSampler) -> ConstraintKind {
        let index = self.dist.sample(&mut literal_sampler.rand);
        match index {
            0 => ConstraintKind::ThreeLiteral,
            1 => ConstraintKind::Cardinality,
            2 => ConstraintKind::ArbitraryLength,
            3 => ConstraintKind::ExactlyOne,
            4 => ConstraintKind::DoublingPairs,
            _ => unreachable!(),
        }
    }
}
