use crate::fuzzer::constraints::constraint::Constraint;
use crate::fuzzer::domain::SolutionModel;
use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;
use crate::fuzzer::pb_settings::GenericConfig;
use crate::fuzzer::{
    constraints::generic::ArbitraryLengthConstraint,
    domain::Comparison,
    generation::{
        builders::degree_range::compute_degree_range,
        generator_settings::{CoeffRange, DegreeRatio},
        literal_sampler::LiteralSampler,
    },
};

#[derive(Copy, Clone)]
pub struct GenericSettings {
    amount_lits: u32,
    coeff_range: CoeffRange,
    degree_ratio: DegreeRatio,
}

impl From<GenericConfig> for GenericSettings {
    fn from(value: GenericConfig) -> Self {
        Self {
            amount_lits: value.amount_lits,
            coeff_range: CoeffRange::new(value.coeff_lower, value.coeff_upper),
            degree_ratio: DegreeRatio::new(value.degree_lower, value.degree_upper),
        }
    }
}

#[derive(Clone, Copy)]
pub struct GenericFromSolutionSettings {
    amount_lits: u32,
    coeff_range: CoeffRange,
}

impl From<GenericConfig> for GenericFromSolutionSettings {
    fn from(value: GenericConfig) -> Self {
        Self {
            amount_lits: value.amount_lits,
            coeff_range: CoeffRange::new(value.coeff_lower, value.coeff_upper),
        }
    }
}

pub fn generate_generic(
    settings: GenericSettings,
    literal_sampler: &mut LiteralSampler,
) -> Result<ArbitraryLengthConstraint, ConstraintGenerationError> {
    let ids = literal_sampler.get_n_ids_sorted(settings.amount_lits)?;
    let (start_val, end_val) = settings.coeff_range.get_bounds();
    let coeffs: Vec<_> = ids
        .iter()
        .map(|_| literal_sampler.sample_without_zero_from_range(start_val, end_val))
        .collect();
    let (start_range, end_range) = compute_degree_range(&coeffs, settings.degree_ratio);

    let comparison = Comparison::GrEqual;
    let degree = literal_sampler.sample_from_range_inclusive(start_range, end_range);
    let literals = ids
        .iter()
        .zip(coeffs)
        .map(|(id, c)| id.to_literal(c))
        .collect();
    Ok(ArbitraryLengthConstraint::new(literals, comparison, degree))
}

pub fn create_sat_generic(
    settings: GenericFromSolutionSettings,
    literal_sampler: &mut LiteralSampler,
    model: &SolutionModel,
) -> Result<ArbitraryLengthConstraint, ConstraintGenerationError> {
    let (start_val, end_val) = settings.coeff_range.get_bounds();
    let amount_lits = settings.amount_lits;
    let lits: Vec<_> = literal_sampler
        .get_n_ids_sorted(amount_lits)?
        .iter()
        .map(|id| id.to_literal(literal_sampler.sample_without_zero_from_range(start_val, end_val)))
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
    Ok(constraint)
}
