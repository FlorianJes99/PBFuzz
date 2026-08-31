use crate::fuzzer::domain::LiteralIdent;
use crate::fuzzer::generation::constraint_errors::ConstraintGenerationError;
use rand::RngExt;
use rand::rngs::StdRng;
use rand::seq::index;

#[derive(Debug)]
pub struct LiteralSampler {
    pub rand: StdRng,
    max_vars: u32,
}

impl LiteralSampler {
    pub fn new(rand: StdRng, max_vars: u32) -> Self {
        Self { rand, max_vars }
    }

    /// samples one value from a given range
    pub(super) fn sample_from_range_inclusive(&mut self, start_range: i64, end_range: i64) -> i64 {
        self.rand.random_range(start_range..=end_range)
    }

    /// Sample either 1 or -1 for the coefficient
    pub(super) fn sample_cardinality_value(&mut self) -> i32 {
        if self.rand.random_bool(0.5) { 1 } else { -1 }
    }

    pub fn sample_without_zero_from_range(&mut self, start_val: i32, end_val: i32) -> i32 {
        loop {
            let rand_sample = self.rand.random_range(start_val..=end_val);
            if rand_sample != 0 {
                if start_val > 0 && self.rand.random_bool(0.5) {
                    // if the range is only positive we want to invert some coefficients
                    return -rand_sample;
                }
                if end_val < 0 && self.rand.random_bool(0.5) {
                    // if the range is only negative we want to invert some coefficients
                    return -rand_sample;
                }
                return rand_sample;
            }
        }
    }

    pub(super) fn get_n_ids_sorted(
        &mut self,
        n: u32,
    ) -> Result<Vec<LiteralIdent>, ConstraintGenerationError> {
        self.get_n_ids(n, true)
    }

    /// Generate n ids to use for creation of literals in a single constraint
    pub(super) fn get_n_ids(
        &mut self,
        n: u32,
        sort: bool,
    ) -> Result<Vec<LiteralIdent>, ConstraintGenerationError> {
        let mut indices: Vec<_> =
            index::sample(&mut self.rand, self.max_vars.try_into()?, n.try_into()?)
                .iter()
                .collect();

        if sort {
            // sort the indices in increasing oder
            indices.sort_unstable();
        }

        indices
            .into_iter()
            .map(|x| -> Result<LiteralIdent, ConstraintGenerationError> {
                // Add +1 because we want ids from 1..=len and sample are from 0..len
                (x + 1)
                    .try_into()
                    .map(LiteralIdent::new)
                    .map_err(Into::into)
            })
            .collect()
    }

    pub(super) fn get_three_ids_sorted(
        &mut self,
    ) -> Result<[LiteralIdent; 3], ConstraintGenerationError> {
        let mut ids: [LiteralIdent; 3] =
            index::sample_array(&mut self.rand, self.max_vars.try_into()?)
                .ok_or(ConstraintGenerationError::InvalidSampling(3, self.max_vars))?
                // Add +1 because we want ids from 1..=len and sample are from 0..len
                .map(|x| x as u32 + 1)
                .map(LiteralIdent::new);
        ids.sort();
        Ok(ids)
    }
}
