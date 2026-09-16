use crate::fuzzer::generation::{
    builders::{
        doubling_constraint_builder::create_sat_doubling_pairs_constraint,
        generic_builder::create_sat_generic,
    },
    constraint_errors::ConstraintGenerationError,
};
use rand::{RngExt, seq::IndexedRandom};

use crate::fuzzer::{
    constraints::constraint::Constraint,
    domain::{LiteralIdent, SolutionModel},
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
    let false_lits: Vec<_> = lits
        .iter()
        .filter(|l| !solution.contains(l))
        .cloned()
        .collect();
    let constraints = (0..settings.amount_constraints)
        .map(|_| sample_sat_constraint(literal_sampler, &settings, &solution, &false_lits))
        .collect::<Result<_, _>>()?;
    Ok(FromSolutionData {
        constraints,
        solution,
    })
}

fn sample_sat_constraint(
    literal_sampler: &mut LiteralSampler,
    settings: &FromSolutionSettings,
    solution: &SolutionModel,
    false_lits: &[LiteralIdent],
) -> Result<Box<dyn Constraint>, ConstraintGenerationError> {
    if literal_sampler.rand.random_bool(0.5) {
        let c = create_sat_doubling_pairs_constraint(
            literal_sampler,
            settings.amount_literals_per_constraint,
            solution,
            false_lits,
        )?;
        Ok(Box::new(c))
    } else {
        let c = create_sat_generic(literal_sampler, settings, solution)?;
        Ok(Box::new(c))
    }
}
