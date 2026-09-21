use rand::RngExt;
use rand::seq::IteratorRandom;

use crate::fuzzer::domain::{LiteralIdent, SolutionModel};
use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;
use crate::fuzzer::pb_settings::DoublingPairsConfig;
use crate::fuzzer::{
    constraints::doubling_pairs::DoublingPairs,
    domain::{Comparison, Literal},
    generation::literal_sampler::LiteralSampler,
};

#[derive(Copy, Clone)]
pub struct DoublingPairsSettings {
    amount_pairs: u32,
}

impl From<DoublingPairsConfig> for DoublingPairsSettings {
    fn from(value: DoublingPairsConfig) -> Self {
        Self {
            amount_pairs: value.amount_pairs,
        }
    }
}

/// Generates constraint with the following structure
///
/// `x_1 + y_1 + 2 x_2 + 2 y_2 + ... + 2^n x_n + 2^n y_n >= 2^(n+1)`
pub fn generate_doubling_pairs_constraint(
    settings: DoublingPairsSettings,
    literal_sampler: &mut LiteralSampler,
) -> Result<DoublingPairs, ConstraintGenerationError> {
    let amount_lits = settings.amount_pairs * 2;
    let ids = literal_sampler.get_n_ids(amount_lits, false)?;
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

pub fn create_sat_doubling_pairs_constraint(
    settings: DoublingPairsSettings,
    literal_sampler: &mut LiteralSampler,
    model: &SolutionModel,
    false_lits: &[LiteralIdent],
) -> Result<DoublingPairs, ConstraintGenerationError> {
    // given those pairs: 1 1 2 2 4 4 8 8 16 16 >= 32
    // we allways want to use a minimum amount of true vars to get as close as possible to 32.
    // the least amount of vars to fulfill the constraint would be 2: 16 16
    // the most amount of vars to fulfill the constraint would be 6 : 16 8 4 2 1 1
    let amount_true_vars = literal_sampler
        .rand
        .random_range(2..=settings.amount_pairs + 1);
    let mut true_vars = model.iter().cloned().sample(
        &mut literal_sampler.rand,
        amount_true_vars.try_into().expect("usize support"),
    );
    let constraint_length = settings.amount_pairs * 2;
    let mut false_vars = false_lits.iter().cloned().sample(
        &mut literal_sampler.rand,
        (constraint_length - amount_true_vars)
            .try_into()
            .expect("usize support"),
    );
    let mut literals = vec![];
    for i in (0..settings.amount_pairs).rev() {
        let c = 2_i64.pow(i);
        let [i1, i2] = if true_vars.len() > 2 {
            [
                true_vars.pop().expect("bug in variable sampling"),
                false_vars.pop().expect("bug in variable sampling"),
            ]
        } else if true_vars.len() == 2 {
            [
                true_vars.pop().expect("bug in variable sampling"),
                true_vars.pop().expect("bug in variable sampling"),
            ]
        } else {
            [
                false_vars.pop().expect("bug in variable sampling"),
                false_vars.pop().expect("bug in variable sampling"),
            ]
        };
        literals.push(i1.to_literal(c.try_into().expect("too large constraint")));
        literals.push(i2.to_literal(c.try_into().expect("too large constraint")));
    }
    // n pairs use exponents 0..=n-1, so highest coefficient is 2^(n-1)
    // therefore RHS needs to be 2^(n-1 + 1) = 2^n
    let degree = 2_i64.pow(settings.amount_pairs);
    Ok(DoublingPairs::new(literals, Comparison::GrEqual, degree))
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::StdRng};

    use crate::fuzzer::constraints::constraint::Constraint;

    use super::*;

    #[test]
    fn test_pairs_constraint() {
        let amount_pairs = 3;
        let settings = DoublingPairsSettings { amount_pairs };
        let mut literal_sampler = LiteralSampler::new(StdRng::seed_from_u64(42), 50);
        let constraint =
            generate_doubling_pairs_constraint(settings, &mut literal_sampler).unwrap();
        assert!(
            constraint.literals().len().is_multiple_of(2),
            "We expect amount of literals in this constraint to be even"
        );
        let literal_size = amount_pairs * 2;
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
