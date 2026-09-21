use crate::fuzzer::domain::Comparison;
use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;

use crate::fuzzer::domain::Literal;

use crate::fuzzer::constraints::exactly_one::ExactlyOneConstraint;
use crate::fuzzer::generation::literal_sampler::LiteralSampler;
use crate::fuzzer::pb_settings::ExactlyOneConfig;

#[derive(Copy, Clone)]
pub struct ExactlyOneSettings {
    amount_lits: u32,
}

impl From<ExactlyOneConfig> for ExactlyOneSettings {
    fn from(value: ExactlyOneConfig) -> Self {
        Self {
            amount_lits: value.amount_lits,
        }
    }
}

pub fn generate_exactly_one_constraint(
    settings: ExactlyOneSettings,
    literal_sampler: &mut LiteralSampler,
) -> Result<ExactlyOneConstraint, ConstraintGenerationError> {
    let literals: Vec<Literal> = literal_sampler
        .get_n_ids_sorted(settings.amount_lits)?
        .iter()
        .map(|id| id.to_literal(1))
        .collect();

    assert!(literals.len() as u32 == settings.amount_lits);
    ExactlyOneConstraint::new(literals, Comparison::Equal, 1).map_err(Into::into)
}
