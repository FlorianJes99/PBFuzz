use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;
use crate::fuzzer::pb_settings::CardinalityConfig;
use crate::fuzzer::{
    constraints::cardinality::CardinalityConstraint,
    domain::Comparison,
    generation::{
        builders::degree_range::compute_degree_range, generator_settings::DegreeRatio,
        literal_sampler::LiteralSampler,
    },
};

#[derive(Clone, Copy)]
pub struct CardinalitySettings {
    amount_lits: u32,
    degree_ratio: DegreeRatio,
}

impl From<CardinalityConfig> for CardinalitySettings {
    fn from(value: CardinalityConfig) -> Self {
        Self {
            amount_lits: value.amount_lits,
            degree_ratio: DegreeRatio::new(value.degree_lower, value.degree_upper),
        }
    }
}

pub fn generate_cardinality(
    settings: CardinalitySettings,
    literal_sampler: &mut LiteralSampler,
) -> Result<CardinalityConstraint, ConstraintGenerationError> {
    let ids = literal_sampler.get_n_ids_sorted(settings.amount_lits)?;
    let coeffs: Vec<_> = ids
        .iter()
        .map(|_| literal_sampler.sample_cardinality_value())
        .collect();
    let (start_range, end_range) = compute_degree_range(&coeffs, settings.degree_ratio);
    let degree = literal_sampler.sample_from_range_inclusive(start_range, end_range);
    let literals = ids
        .iter()
        .zip(coeffs)
        .map(|(id, c)| id.to_literal(c))
        .collect();
    CardinalityConstraint::new(literals, Comparison::GrEqual, degree).map_err(Into::into)
}
