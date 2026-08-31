use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;
use crate::fuzzer::{
    constraints::generic::ArbitraryLengthConstraint,
    domain::Comparison,
    generation::{
        builders::degree_range::compute_degree_range,
        generator_settings::{CoeffRange, DegreeRatio},
        literal_sampler::LiteralSampler,
    },
};

pub fn generate_generic(
    literal_sampler: &mut LiteralSampler,
    coeff_range: CoeffRange,
    degree_ratio: DegreeRatio,
    length: u32,
) -> Result<ArbitraryLengthConstraint, ConstraintGenerationError> {
    let ids = literal_sampler.get_n_ids_sorted(length)?;
    let (start_val, end_val) = coeff_range.get_bounds();
    let coeffs: Vec<_> = ids
        .iter()
        .map(|_| literal_sampler.sample_without_zero_from_range(start_val, end_val))
        .collect();
    let (start_range, end_range) = compute_degree_range(&coeffs, degree_ratio);

    let comparison = Comparison::GrEqual;
    let degree = literal_sampler.sample_from_range_inclusive(start_range, end_range);
    let literals = ids
        .iter()
        .zip(coeffs)
        .map(|(id, c)| id.to_literal(c))
        .collect();
    Ok(ArbitraryLengthConstraint::new(literals, comparison, degree))
}
