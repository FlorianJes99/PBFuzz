use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;
use crate::fuzzer::{
    constraints::three_literals::ThreeLiteralConstraint,
    domain::Comparison,
    generation::{
        builders::degree_range::compute_degree_range,
        generator_settings::{CoeffRange, DegreeRatio},
        literal_sampler::LiteralSampler,
    },
};

pub fn generate_three_literal(
    literal_sampler: &mut LiteralSampler,
    coeff_range: CoeffRange,
    degree_ratio: DegreeRatio,
) -> Result<ThreeLiteralConstraint, ConstraintGenerationError> {
    let (start_val, end_val) = coeff_range.get_bounds();
    let ids = literal_sampler.get_three_ids_sorted()?;
    let coeffs = ids.map(|_| literal_sampler.sample_without_zero_from_range(start_val, end_val));
    let (start_range, end_range) = compute_degree_range(&coeffs, degree_ratio);
    let literals = std::array::from_fn(|i| ids[i].to_literal(coeffs[i]));
    let comparison = Comparison::GrEqual;
    Ok(ThreeLiteralConstraint::new(
        literals,
        comparison,
        literal_sampler.sample_from_range_inclusive(start_range, end_range),
    ))
}
