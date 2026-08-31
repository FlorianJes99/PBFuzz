use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;
use crate::fuzzer::{
    constraints::doubling_pairs::DoublingPairs,
    domain::{Comparison, Literal},
    generation::literal_sampler::LiteralSampler,
};

/// Generates constraint with the following structure
///
/// `x_1 + y_1 + 2 x_2 + 2 y_2 + ... + 2^n x_n + 2^n y_n >= 2^(n+1)`
pub fn generate_doubling_pairs_constraint(
    literal_sampler: &mut LiteralSampler,
    constraint_length: u32,
) -> Result<DoublingPairs, ConstraintGenerationError> {
    assert!(
        constraint_length.is_multiple_of(2),
        "This type of constraint expects to have an even amount of literals"
    );
    let ids = literal_sampler.get_n_ids(constraint_length, false)?;
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

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::StdRng};

    use crate::fuzzer::constraints::constraint::Constraint;

    use super::*;

    #[test]
    fn test_pairs_constraint() {
        let literal_size = 6;
        let mut literal_sampler = LiteralSampler::new(StdRng::seed_from_u64(42), 50);
        let constraint =
            generate_doubling_pairs_constraint(&mut literal_sampler, literal_size).unwrap();
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
