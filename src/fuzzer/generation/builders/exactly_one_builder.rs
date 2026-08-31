use crate::fuzzer::domain::Comparison;
use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;

use crate::fuzzer::domain::Literal;

use crate::fuzzer::constraints::exactly_one::ExactlyOneConstraint;
use crate::fuzzer::generation::literal_sampler::LiteralSampler;

pub fn generate_exactly_one_constraint(
    literal_sampler: &mut LiteralSampler,
    length: u32,
) -> Result<ExactlyOneConstraint, ConstraintGenerationError> {
    let literals: Vec<Literal> = literal_sampler
        .get_n_ids_sorted(length)?
        .iter()
        .map(|id| id.to_literal(1))
        .collect();

    assert!(literals.len() as u32 == length);
    ExactlyOneConstraint::new(literals, Comparison::Equal, 1).map_err(Into::into)
}
