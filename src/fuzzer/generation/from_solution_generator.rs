use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;
use rand::seq::IndexedRandom;

use crate::fuzzer::{
    constraints::{constraint::Constraint, generic::ArbitraryLengthConstraint},
    domain::{Comparison, LiteralIdent, SolutionModel},
    generation::{generator_settings::CoeffRange, literal_sampler::LiteralSampler},
    pb_settings::GeneratorSettings,
};

pub struct FromSolutionData {
    pub constraints: Vec<Box<dyn Constraint>>,
    pub solution: SolutionModel,
}

pub struct FromSolutionSettings {
    pub amount_constraints: u64,
    pub max_vars: u32,
    pub amount_literals_per_constraint: u32,
    pub coeff_range: CoeffRange,
}

impl FromSolutionSettings {
    pub fn create_from(generator_settings: GeneratorSettings, amount_constraints: u64) -> Self {
        Self {
            amount_constraints,
            max_vars: generator_settings.max_vars,
            amount_literals_per_constraint: generator_settings.amount_literals_per_constraint,
            coeff_range: CoeffRange::new(
                generator_settings.coeff_lower_bound,
                generator_settings.coeff_upper_bound,
            ),
        }
    }
}

/// Generate constraints to a given assignemnts of the literals
///
/// Instead of sampling random constraints, first sample a subset of literals, which will be assigned
/// to true. Then we only generate constraints, which are satisfiable under the given solution.
pub fn generate_from_solution(
    literal_sampler: &mut LiteralSampler,
    settings: FromSolutionSettings,
) -> Result<FromSolutionData, ConstraintGenerationError> {
    let amount_vars = settings.max_vars;
    let lits: Vec<_> = (1..=amount_vars).map(LiteralIdent::new).collect();
    let solution: SolutionModel = lits
        .sample(&mut literal_sampler.rand, (amount_vars / 3) as usize)
        .cloned()
        .into();
    let constraints = (0..settings.amount_constraints)
        .map(|_| create_sat_constraint(literal_sampler, &settings, &solution))
        .collect::<Result<_, _>>()?;
    Ok(FromSolutionData {
        constraints,
        solution,
    })
}

fn create_sat_constraint(
    literal_sampler: &mut LiteralSampler,
    settings: &FromSolutionSettings,
    model: &SolutionModel,
) -> Result<Box<dyn Constraint>, ConstraintGenerationError> {
    let (start_val, end_val) = settings.coeff_range.get_bounds();
    let amount_lits = settings.amount_literals_per_constraint;
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
    Ok(Box::new(constraint))
}
