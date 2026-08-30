use itertools::Itertools;
use log::debug;
use rand::{
    RngExt,
    distr::{Distribution, weighted::WeightedIndex},
    rngs::StdRng,
    seq::IndexedRandom,
};
use std::num::TryFromIntError;
use thiserror::Error;

use crate::fuzzer::{
    constraints::{
        cardinality::CardinalityConstraint,
        constraint::{Constraint, ConstraintKind, CostraintWithUnsatAssignments},
        doubling_pairs::DoublingPairs,
        errors::InvalidConstraintConstruction,
        exactly_one::ExactlyOneConstraint,
        generic::ArbitraryLengthConstraint,
        three_literals::ThreeLiteralConstraint,
    },
    domain::{Comparison, Literal, LiteralIdent, MinFunction, SolutionModel},
    generation::{
        generator_settings::{ConstraintSettings, DegreeRatio},
        literal_sampler::LiteralSampler,
    },
};

#[derive(Debug)]
pub struct ConstraintGenerator {
    literal_sampler: LiteralSampler,
    dist: WeightedIndex<u32>,
    settings: ConstraintSettings,
}

impl ConstraintGenerator {
    pub fn new(rand: StdRng, settings: ConstraintSettings) -> Self {
        let dist = WeightedIndex::new(settings.constraint_distribution.get_weights())
            .expect("We should only create valid distributions, when sampling constraint types");
        let literal_sampler = LiteralSampler::new(rand, settings.max_vars);
        Self {
            literal_sampler,
            dist,
            settings,
        }
    }

    fn sample_constraint_kind(&mut self) -> ConstraintKind {
        let index = self.dist.sample(&mut self.literal_sampler.rand);
        match index {
            0 => ConstraintKind::ThreeLiteral,
            1 => ConstraintKind::Cardinality,
            2 => ConstraintKind::ArbitraryLength,
            3 => ConstraintKind::ExactlyOne,
            4 => ConstraintKind::DoublingPairs,
            _ => unreachable!(),
        }
    }

    pub fn generate_random_constraint_with_unsat_count(
        &mut self,
    ) -> Result<Box<dyn CostraintWithUnsatAssignments>, ConstraintGenerationError> {
        if self.literal_sampler.rand.random_bool(0.5) {
            let mut c = self.generate_three_literal()?;
            // Assert we always return a constraint which fulfills max unsat assignments requirement
            let unsat_single_constraint: u128 =
                self.settings.max_unsat_for_single_constraint.into();
            while c.amount_unsat_assignments() > unsat_single_constraint {
                c = self.generate_three_literal()?;
            }
            Ok(Box::new(c))
        } else {
            let amount_literals = self.literal_sampler.rand.random_range(5..=20);
            let c = self.generate_cardinality(amount_literals)?;
            Ok(Box::new(c))
        }
    }
    pub fn generate_random_constraint(
        &mut self,
    ) -> Result<Box<dyn Constraint>, ConstraintGenerationError> {
        let kind = self.sample_constraint_kind();
        match kind {
            ConstraintKind::ThreeLiteral => {
                let mut c = self.generate_three_literal()?;
                // Assert we always return a constraint which fulfills max unsat assignments requirement
                while c.amount_unsat_assignments()
                    > std::convert::Into::<u128>::into(
                        self.settings.max_unsat_for_single_constraint,
                    )
                {
                    debug!("Looping for valid three literal constraint");
                    c = self.generate_three_literal()?;
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
                let c = self.generate_cardinality(constraint_len)?;
                Ok(Box::new(c))
            }
            ConstraintKind::ArbitraryLength => {
                let constraint_len =
                    if self.settings.max_vars <= self.settings.amount_literals_per_constraint {
                        (self.settings.max_vars as f32 * 0.5).floor() as u32
                    } else {
                        self.settings.amount_literals_per_constraint
                    };
                let c = self.generate_generic(constraint_len)?;
                Ok(Box::new(c))
            }
            ConstraintKind::ExactlyOne => {
                // we want to have at max 3 literals in the exactly one constraint for now
                let constraint_len = 3.min((self.settings.max_vars as f32 * 0.5).floor() as u32);
                let c = self.generate_exactly_one_constraint(constraint_len)?;
                Ok(Box::new(c))
            }
            ConstraintKind::DoublingPairs => {
                let n = 2; // TODO: determine length here
                let c = self.generate_doubling_pairs_constraint(n)?;
                Ok(Box::new(c))
            }
        }
    }

    pub fn generate_generic(
        &mut self,
        length: u32,
    ) -> Result<ArbitraryLengthConstraint, ConstraintGenerationError> {
        let ids = self.literal_sampler.get_n_ids_sorted(length)?;
        let (start_val, end_val) = self.settings.coeff_range.get_bounds();
        let coeffs: Vec<_> = ids
            .iter()
            .map(|_| {
                self.literal_sampler
                    .sample_without_zero_from_range(start_val, end_val)
            })
            .collect();
        let (start_range, end_range) = compute_degree_range(&coeffs, self.settings.degree_ratio);

        let comparison = Comparison::GrEqual;
        let degree = self
            .literal_sampler
            .sample_from_range_inclusive(start_range, end_range);
        let literals = ids
            .iter()
            .zip(coeffs)
            .map(|(id, c)| id.to_literal(c))
            .collect();
        Ok(ArbitraryLengthConstraint::new(literals, comparison, degree))
    }

    pub fn generate_cardinality(
        &mut self,
        length: u32,
    ) -> Result<CardinalityConstraint, ConstraintGenerationError> {
        let ids = self.literal_sampler.get_n_ids_sorted(length)?;
        let coeffs: Vec<_> = ids
            .iter()
            .map(|_| self.literal_sampler.sample_cardinality_value())
            .collect();
        let (start_range, end_range) = compute_degree_range(&coeffs, self.settings.degree_ratio);
        let degree = self
            .literal_sampler
            .sample_from_range_inclusive(start_range, end_range);
        let literals = ids
            .iter()
            .zip(coeffs)
            .map(|(id, c)| id.to_literal(c))
            .collect();
        CardinalityConstraint::new(literals, Comparison::GrEqual, degree).map_err(Into::into)
    }

    pub fn generate_three_literal(
        &mut self,
    ) -> Result<ThreeLiteralConstraint, ConstraintGenerationError> {
        let (start_val, end_val) = self.settings.coeff_range.get_bounds();
        let ids = self.literal_sampler.get_three_ids_sorted()?;
        let coeffs = ids.map(|_| {
            self.literal_sampler
                .sample_without_zero_from_range(start_val, end_val)
        });
        let (start_range, end_range) = compute_degree_range(&coeffs, self.settings.degree_ratio);
        let literals = std::array::from_fn(|i| ids[i].to_literal(coeffs[i]));
        let comparison = Comparison::GrEqual;
        Ok(ThreeLiteralConstraint::new(
            literals,
            comparison,
            self.literal_sampler
                .sample_from_range_inclusive(start_range, end_range),
        ))
    }

    pub fn generate_exactly_one_constraint(
        &mut self,
        length: u32,
    ) -> Result<ExactlyOneConstraint, ConstraintGenerationError> {
        let literals: Vec<Literal> = self
            .literal_sampler
            .get_n_ids_sorted(length)?
            .iter()
            .map(|id| id.to_literal(1))
            .collect();

        assert!(literals.len() as u32 == length);
        ExactlyOneConstraint::new(literals, Comparison::Equal, 1).map_err(Into::into)
    }

    /// Generate constraints to a given assignemnts of the literals
    ///
    /// Instead of sampling random constraints, first sample a subset of literals, which will be assigned
    /// to true. Then we only generate constraints, which are satisfiable under the given solution.
    pub fn generate_from_solution(
        &mut self,
        amount_constraints: u64,
    ) -> Result<FromSolutionData, ConstraintGenerationError> {
        let amount_vars = self.settings.max_vars;
        let lits: Vec<_> = (1..=amount_vars).map(LiteralIdent::new).collect();
        let solution: SolutionModel = lits
            .sample(&mut self.literal_sampler.rand, (amount_vars / 3) as usize)
            .cloned()
            .into();
        let constraints = (0..amount_constraints)
            .map(|_| self.create_sat_constraint(&solution))
            .collect::<Result<_, _>>()?;
        Ok(FromSolutionData {
            constraints,
            solution,
        })
    }

    fn create_sat_constraint(
        &mut self,
        model: &SolutionModel,
    ) -> Result<Box<dyn Constraint>, ConstraintGenerationError> {
        let (start_val, end_val) = self.settings.coeff_range.get_bounds();
        let amount_lits = self.settings.amount_literals_per_constraint;
        let lits: Vec<_> = self
            .literal_sampler
            .get_n_ids_sorted(amount_lits)?
            .iter()
            .map(|id| {
                id.to_literal(
                    self.literal_sampler
                        .sample_without_zero_from_range(start_val, end_val),
                )
            })
            .collect();
        let comparison = Comparison::GrEqual;
        let (min_lit, sum_lit) = lits
            .iter()
            .filter(|l| model.contains(&l.literal_id))
            .map(|l| i64::from(l.coeff))
            .fold((0, 0), |acc, c| (acc.0.min(c), acc.1 + c));
        let degree = min_lit.min(sum_lit);
        let constraint = ArbitraryLengthConstraint::new(lits, comparison, degree);
        assert!(
            constraint.verify(model).is_ok(),
            "When generating from a sampled solution, every constraint needs to be satisfiable\nModel: {model}\nConstraint: {constraint}"
        );
        Ok(Box::new(constraint))
    }

    pub fn generate_min_function(
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

    /// Generates constraint with the following structure
    ///
    /// `x_1 + y_1 + 2 x_2 + 2 y_2 + ... + 2^n x_n + 2^n y_n >= 2^(n+1)`
    fn generate_doubling_pairs_constraint(
        &mut self,
        constraint_length: u32,
    ) -> Result<DoublingPairs, ConstraintGenerationError> {
        assert!(
            constraint_length.is_multiple_of(2),
            "This type of constraint expects to have an even amount of literals"
        );
        let ids = self.literal_sampler.get_n_ids(constraint_length, false)?;
        let n: u32 = (ids.len() / 2).try_into()?;
        let literals = ids
            .chunks(2)
            .enumerate()
            .flat_map(|(exponent, chunk)| {
                chunk
                    .iter()
                    .map(move |id| -> Result<Literal, ConstraintGenerationError> {
                        Ok(id.to_literal(2_i32.pow(exponent.try_into()?)))
                    })
            })
            .collect::<Result<_, _>>()?;
        // n pairs use exponents 0..=n-1, so highest coefficient is 2^(n-1)
        // therefore RHS needs to be 2^(n-1 + 1) = 2^n
        let degree = 2_i64.pow(n);
        Ok(DoublingPairs::new(literals, Comparison::GrEqual, degree))
    }
}

/// Finds the lower and upper bound values in a list of coefficients
///
/// Given n coeffs and a fractional lower and upper bound, determine the minimum
/// and maximum value of a range
fn compute_degree_range(coeffs: &[i32], degree_ratio: DegreeRatio) -> (i64, i64) {
    assert!(!coeffs.is_empty());
    let sum_negative: i64 = coeffs.iter().filter(|&&c| c < 0).map(|&c| c as i64).sum();
    let sum_positive: i64 = coeffs.iter().filter(|&&c| c > 0).map(|&c| c as i64).sum();
    let distance = (sum_positive - sum_negative).abs() as f64;
    let (lower_bound_degree, upper_bound_degree) = degree_ratio.get_bounds();
    let range_start = sum_negative + (lower_bound_degree * distance).floor() as i64;
    let range_end = sum_negative + (upper_bound_degree * distance).ceil() as i64;

    assert!(
        range_start <= range_end,
        "Range start should be smaller than / equal to range end"
    );
    (range_start, range_end)
}

pub struct FromSolutionData {
    pub constraints: Vec<Box<dyn Constraint>>,
    pub solution: SolutionModel,
}

/// Sum type for all Errors happenings during Constraint Generation
///
/// This can be either Errors wrapped and propagated upwards from underlying calls or new error types
#[derive(Debug, Error)]
pub enum ConstraintGenerationError {
    #[error("Construction of constraint failed")]
    Invalid(#[from] InvalidConstraintConstruction),
    #[error(transparent)]
    ConvertionFromInt(#[from] TryFromIntError),
    #[error("Constraint length ({0}) is larger than amount of distinct literal ids ({1})")]
    InvalidSampling(u32, u32),
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::fuzzer::generation::generator_settings::{
        CoeffRange, ConstraintDistribution, DegreeRatio,
    };
    use rand::SeedableRng;

    #[test]
    fn test_range_bounds() {
        let degree_ratio = DegreeRatio::new(0.25, 0.5);
        let coeffs = vec![-4, -2, 6, 8];
        let (start, end) = compute_degree_range(&coeffs, degree_ratio);
        println!("{}..={}", start, end);
        assert!(!(start..=end).contains(&-2));
        assert!((start..=end).contains(&-1));

        assert!((start..=end).contains(&4));
        assert!(!(start..=end).contains(&5));
    }

    #[test]
    fn test_pairs_constraint() {
        let settings = ConstraintSettings {
            max_vars: 50,
            max_unsat_for_single_constraint: 3,
            amount_literals_per_constraint: 10,
            coeff_range: CoeffRange::new(2, 5),
            degree_ratio: DegreeRatio::new(0.3, 0.5),
            constraint_distribution: ConstraintDistribution {
                three_literal_weight: 3,
                cardinality_weight: 1,
                arbitrary_weight: 1,
                exactly_one_weight: 0,
                doubling_pairs_weight: 1,
            },
        };
        let mut generator = ConstraintGenerator::new(StdRng::seed_from_u64(42), settings);
        let literal_size = 6;
        let constraint = generator
            .generate_doubling_pairs_constraint(literal_size)
            .unwrap();
        assert!(
            constraint.literals().len().is_multiple_of(2),
            "We expect amount of literals in this constraint to be even"
        );
        assert_eq!(constraint.literals().len(), literal_size as usize);
        assert_eq!(constraint.degree(), 2_i64.pow(literal_size / 2));
        constraint
            .literals()
            .chunks(2)
            .enumerate()
            .for_each(|(i, chunk)| {
                let [l1, l2] = chunk else {
                    panic!("Expected pairs to be of length 2");
                };

                let (c1, c2) = (l1.coeff, l2.coeff);
                let expected = 2_i32.pow(i as u32);
                assert_eq!(c1, expected, "Expected {expected}, but {c1}");
                assert_eq!(
                    c1, c2,
                    "Both coeffs of a pair should be equal, but was: {} and {}",
                    c1, c2
                )
            });
    }
}
