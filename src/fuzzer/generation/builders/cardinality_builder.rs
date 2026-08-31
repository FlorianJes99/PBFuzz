use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;
use crate::fuzzer::{
    constraints::cardinality::CardinalityConstraint,
    domain::Comparison,
    generation::{
        builders::degree_range::compute_degree_range, generator_settings::DegreeRatio,
        literal_sampler::LiteralSampler,
    },
};

pub fn generate_cardinality(
    literal_sampler: &mut LiteralSampler,
    degree_ratio: DegreeRatio,
    length: u32,
) -> Result<CardinalityConstraint, ConstraintGenerationError> {
    let ids = literal_sampler.get_n_ids_sorted(length)?;
    let coeffs: Vec<_> = ids
        .iter()
        .map(|_| literal_sampler.sample_cardinality_value())
        .collect();
    let (start_range, end_range) = compute_degree_range(&coeffs, degree_ratio);
    let degree = literal_sampler.sample_from_range_inclusive(start_range, end_range);
    let literals = ids
        .iter()
        .zip(coeffs)
        .map(|(id, c)| id.to_literal(c))
        .collect();
    CardinalityConstraint::new(literals, Comparison::GrEqual, degree).map_err(Into::into)
}
