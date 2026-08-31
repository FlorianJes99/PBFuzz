use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;
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
        generator_settings::ConstraintSettings,
        literal_sampler::LiteralSampler,
    },
};

pub struct RandomConstraintGenerator {
    settings: ConstraintSettings,
    dist: WeightedIndex<u32>,
}

impl RandomConstraintGenerator {
    pub fn new(settings: ConstraintSettings) -> Self {
        let dist = WeightedIndex::new(settings.constraint_distribution.get_weights())
            .expect("We should only create valid distributions, when sampling constraint types");
        Self { settings, dist }
    }

    pub fn generate_random_constraint_with_unsat_count(
        &self,
        literal_sampler: &mut LiteralSampler,
    ) -> Result<Box<dyn CostraintWithUnsatAssignments>, ConstraintGenerationError> {
        if literal_sampler.rand.random_bool(0.5) {
            let mut c = generate_three_literal(
                literal_sampler,
                self.settings.coeff_range,
                self.settings.degree_ratio,
            )?;
            // Assert we always return a constraint which fulfills max unsat assignments requirement
            let unsat_single_constraint: u128 =
                self.settings.max_unsat_for_single_constraint.into();
            while c.amount_unsat_assignments() > unsat_single_constraint {
                c = generate_three_literal(
                    literal_sampler,
                    self.settings.coeff_range,
                    self.settings.degree_ratio,
                )?;
            }
            Ok(Box::new(c))
        } else {
            let amount_literals = literal_sampler.rand.random_range(5..=20);
            let c =
                generate_cardinality(literal_sampler, self.settings.degree_ratio, amount_literals)?;
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
                let mut c = generate_three_literal(
                    literal_sampler,
                    self.settings.coeff_range,
                    self.settings.degree_ratio,
                )?;
                // Assert we always return a constraint which fulfills max unsat assignments requirement
                while c.amount_unsat_assignments()
                    > std::convert::Into::<u128>::into(
                        self.settings.max_unsat_for_single_constraint,
                    )
                {
                    debug!("Looping for valid three literal constraint");
                    c = generate_three_literal(
                        literal_sampler,
                        self.settings.coeff_range,
                        self.settings.degree_ratio,
                    )?;
                }
                Ok(Box::new(c))
            }
            ConstraintKind::Cardinality => {
                let constraint_len =
                    if self.settings.max_vars <= self.settings.amount_literals_per_constraint {
                        (self.settings.max_vars as f32 * 0.5).floor() as u32
                    } else {
                        self.settings.amount_literals_per_constraint
                    };
                let c = generate_cardinality(
                    literal_sampler,
                    self.settings.degree_ratio,
                    constraint_len,
                )?;
                Ok(Box::new(c))
            }
            ConstraintKind::ArbitraryLength => {
                let constraint_len =
                    if self.settings.max_vars <= self.settings.amount_literals_per_constraint {
                        (self.settings.max_vars as f32 * 0.5).floor() as u32
                    } else {
                        self.settings.amount_literals_per_constraint
                    };
                let c = generate_generic(
                    literal_sampler,
                    self.settings.coeff_range,
                    self.settings.degree_ratio,
                    constraint_len,
                )?;
                Ok(Box::new(c))
            }
            ConstraintKind::ExactlyOne => {
                // we want to have at max 3 literals in the exactly one constraint for now
                let constraint_len = 3.min((self.settings.max_vars as f32 * 0.5).floor() as u32);
                let c = generate_exactly_one_constraint(literal_sampler, constraint_len)?;
                Ok(Box::new(c))
            }
            ConstraintKind::DoublingPairs => {
                let n = 2; // TODO: determine length here
                let c = generate_doubling_pairs_constraint(literal_sampler, n)?;
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
