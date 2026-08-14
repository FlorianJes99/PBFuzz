use itertools::Itertools;
use log::debug;
use rand::{
    RngExt,
    distr::{Distribution, weighted::WeightedIndex},
    rngs::StdRng,
    seq::{IndexedRandom, index},
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
    generation::generator_settings::ConstraintSettings,
};

#[derive(Debug)]
pub struct ConstraintGenerator {
    rand: StdRng,
    dist: WeightedIndex<i32>,
    settings: ConstraintSettings,
}

impl ConstraintGenerator {
    pub fn new(rand: StdRng, settings: ConstraintSettings) -> Self {
        let dist = WeightedIndex::new(settings.constraint_distribution.get_weights())
            .expect("We should only create valid distributions, when sampling constraint types");
        Self {
            rand,
            dist,
            settings,
        }
    }

    fn sample_constraint_kind(&mut self) -> ConstraintKind {
        let index = self.dist.sample(&mut self.rand);
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
        if self.rand.random_bool(0.5) {
            let mut c = self.generate_three_literal()?;
            // Assert we always return a constraint which fulfills max unsat assignments requirement
            let unsat_single_constraint: u128 =
                self.settings.max_unsat_for_single_constraint.into();
            while c.amount_unsat_assignments() > unsat_single_constraint {
                c = self.generate_three_literal()?;
            }
            Ok(Box::new(c))
        } else {
            let amount_literals = self.rand.random_range(5..=20);
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
        assert!(length != 0, "Length of constraint needs to be > 0");
        let ids = self.get_n_ids_sorted(length)?;
        let (start_val, end_val) = self.settings.coeff_range.get_bounds();
        let mut literals = vec![];
        let mut min_coeff = i64::MAX;
        let mut max_coeff = i64::MIN;
        let mut min_sum_coeff = 0;
        for id in &ids {
            let coeff = self.sample_without_zero_from_range(start_val, end_val);
            literals.push(id.to_literal(coeff));

            let coeff = coeff.into();

            if coeff < 0 {
                min_sum_coeff += coeff;
            }
            if coeff > max_coeff {
                max_coeff = coeff;
            }
            if coeff < min_coeff {
                min_coeff = coeff
            }
        }
        let distance = (max_coeff - min_coeff).abs() as f64;
        let (lower_bound_degree, upper_bound_degree) = self.settings.degree_ratio.get_bounds();
        let range_start = min_sum_coeff + (lower_bound_degree * distance).floor() as i64;
        let range_end = min_coeff + (upper_bound_degree * distance).ceil() as i64;
        let range_start = min_coeff.max(range_start);
        // This ensures that we have always a single literal, which can already fulfill the equation
        let range_end = max_coeff.min(range_end);
        assert!(
            range_start <= range_end,
            "Range start should be smaller than / equal to range end"
        );

        let comparison = Comparison::GrEqual;
        let degree = self.rand.random_range(range_start..=range_end);
        Ok(ArbitraryLengthConstraint::new(literals, comparison, degree))
    }

    pub fn generate_cardinality(
        &mut self,
        length: u32,
    ) -> Result<CardinalityConstraint, ConstraintGenerationError> {
        let ids = self.get_n_ids_sorted(length)?;
        assert!(ids.len() as u32 == length);
        let mut literals = vec![];
        let mut lower_max: i32 = 0;
        let mut upper_max: i32 = 0;
        for id in &ids {
            if self.rand.random_bool(0.5) {
                literals.push(id.to_literal(1));
                upper_max += 1;
            } else {
                literals.push(id.to_literal(-1));
                lower_max -= 1;
            }
        }
        assert!(literals.len() as u32 == length);
        let distance = upper_max - lower_max;
        let (lower_bound_degree, upper_bound_degree) = self.settings.degree_ratio.get_bounds();
        let lower_bound = lower_max + (lower_bound_degree * distance as f64).ceil() as i32;
        let upper_bound = lower_max + (upper_bound_degree * distance as f64).floor() as i32;
        let degree = self.rand.random_range(lower_bound..upper_bound).into();
        CardinalityConstraint::new(literals, Comparison::GrEqual, degree).map_err(Into::into)
    }

    pub fn generate_three_literal(
        &mut self,
    ) -> Result<ThreeLiteralConstraint, ConstraintGenerationError> {
        let ids = self.get_three_ids_sorted()?;
        let comparison = Comparison::GrEqual;
        let (start_val, end_val) = self.settings.coeff_range.get_bounds();
        let mut literals = vec![];
        let mut min_coeff = i64::MAX;
        let mut max_coeff = i64::MIN;
        let mut min_sum_coeff = 0;
        for id in &ids {
            let coeff = self.sample_without_zero_from_range(start_val, end_val);
            literals.push(id.to_literal(coeff));

            let coeff = coeff.into();

            if coeff < 0 {
                min_sum_coeff += coeff;
            }
            if coeff > max_coeff {
                max_coeff = coeff;
            }
            if coeff < min_coeff {
                min_coeff = coeff
            }
        }
        let distance = (max_coeff - min_coeff).abs() as f64;
        let (lower_bound_degree, upper_bound_degree) = self.settings.degree_ratio.get_bounds();
        let range_start = min_sum_coeff + (lower_bound_degree * distance).floor() as i64;
        let range_end = min_coeff + (upper_bound_degree * distance).ceil() as i64;
        let range_start = min_coeff.max(range_start);
        // This ensures that we have always a single literal, which can already fulfill the equation
        let range_end = max_coeff.min(range_end);
        assert!(
            range_start <= range_end,
            "Range start should be smaller than / equal to range end"
        );
        let literals = literals.try_into().expect("Unreachable");
        Ok(ThreeLiteralConstraint::new(
            literals,
            comparison,
            self.rand.random_range(range_start..=range_end),
        ))
    }

    pub fn generate_exactly_one_constraint(
        &mut self,
        length: u32,
    ) -> Result<ExactlyOneConstraint, ConstraintGenerationError> {
        let literals: Vec<Literal> = self
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
            .sample(&mut self.rand, (amount_vars / 3) as usize)
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
            .get_n_ids_sorted(amount_lits)?
            .iter()
            .map(|id| id.to_literal(self.sample_without_zero_from_range(start_val, end_val)))
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
            .sample(&mut self.rand, amount)
            .sorted()
            .map(|id| id.to_literal(self.sample_without_zero_from_range(-3, 3)))
            .collect();
        MinFunction::new(literals)
    }

    fn sample_without_zero_from_range(&mut self, start_val: i32, end_val: i32) -> i32 {
        loop {
            let rand_sample = self.rand.random_range(start_val..=end_val);
            if rand_sample != 0 {
                if start_val > 0 && self.rand.random_bool(0.5) {
                    // if the range is only positive we want to invert some coefficients
                    return -rand_sample;
                }
                if end_val < 0 && self.rand.random_bool(0.5) {
                    // if the range is only negative we want to invert some coefficients
                    return -rand_sample;
                }
                return rand_sample;
            }
        }
    }

    fn get_n_ids_sorted(&mut self, n: u32) -> Result<Vec<LiteralIdent>, ConstraintGenerationError> {
        self.get_n_ids(n, true)
    }

    /// Generate n ids to use for creation of literals in a single constraint
    fn get_n_ids(
        &mut self,
        n: u32,
        sort: bool,
    ) -> Result<Vec<LiteralIdent>, ConstraintGenerationError> {
        let mut indices: Vec<_> = index::sample(
            &mut self.rand,
            self.settings.max_vars.try_into()?,
            n.try_into()?,
        )
        .iter()
        .collect();

        if sort {
            // sort the indices in increasing oder
            indices.sort_unstable();
        }

        indices
            .into_iter()
            .map(|x| -> Result<LiteralIdent, ConstraintGenerationError> {
                // Add +1 because we want ids from 1..=len and sample are from 0..len
                (x + 1)
                    .try_into()
                    .map(LiteralIdent::new)
                    .map_err(Into::into)
            })
            .collect()
    }

    fn get_three_ids_sorted(&mut self) -> Result<[LiteralIdent; 3], ConstraintGenerationError> {
        let mut ids: [LiteralIdent; 3] =
            index::sample_array(&mut self.rand, self.settings.max_vars.try_into()?)
                .ok_or(ConstraintGenerationError::InvalidSampling(
                    3,
                    self.settings.max_vars,
                ))?
                // Add +1 because we want ids from 1..=len and sample are from 0..len
                .map(|x| x as u32 + 1)
                .map(LiteralIdent::new);
        ids.sort();
        Ok(ids)
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
        let ids = self.get_n_ids(constraint_length, false)?;
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

pub struct FromSolutionData {
    pub constraints: Vec<Box<dyn Constraint>>,
    pub solution: SolutionModel,
}

/// Sum type for all Errors happenings during Constraint Generation
///
/// This can be either Errors wrapped and propagated upwards from underlying calls or new error types
#[derive(Debug, Error)]
pub enum ConstraintGenerationError {
    #[error("Subtraction failed due to overflow: {0} - {1}")]
    SubtractionOverflow(i64, i64),
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
