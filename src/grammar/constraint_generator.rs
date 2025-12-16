use rand::{Rng, rngs::StdRng};
use std::i64;

use crate::grammar::{
    constraint::{
        ArbitraryLengthConstraint, CardinalityConstraint, Constraint, ThreeAtomConstraint,
    },
    domain::{AtomIdent, Comparison},
};

#[derive(Debug)]
pub enum ConstraintType {
    Generic { ids: Vec<AtomIdent> },
    Cardinality { ids: Vec<AtomIdent> },
    ThreeAtom { ids: [AtomIdent; 3] },
}

#[derive(Debug)]
pub struct ConstraintGenerator {
    settings: ConstraintSettings,
}

impl ConstraintGenerator {
    pub fn new(settings: ConstraintSettings) -> Self {
        Self { settings }
    }


}

#[derive(Debug)]
pub struct ConstraintSettings {
    max_unsat_for_single_constraint: u32,
    /// The possible values a coefficient of a atom can be
    /// E.g.: if `coeff_range = 8`, then values can range from -8..8 except 0
    coeff_range: i32,
    /// Given the sampled atoms in a constraint, the lower bound specifies the smallest possible
    /// fraction of the given range
    /// E.g.: If possible assignments yield a range of 0..10 and `lower_bound_degree = 0.3`, then
    ///       lowest possible degree (RHS) can be 3
    lower_bound_degree: f64,
    /// Given the sampled atoms in a constraint, the upper bound specifies the highest possible
    /// fraction of the given range
    /// E.g.: If possible assignments yield a range of 0..10 and `upper_bound_degree = 0.6`, then
    ///       highest possible degree (RHS) can be 6
    upper_bound_degree: f64,
}

impl ConstraintSettings {
    pub fn new(max_unsat_for_single_constraint: u32, coeff_range: i32, lower_bound_degree: f64, upper_bound_degree: f64) -> Self {
        Self { max_unsat_for_single_constraint, coeff_range, lower_bound_degree, upper_bound_degree }
    }
}

impl ConstraintGenerator {
    pub fn generate_constraint(
        &self,
        rand: &mut StdRng,
        ct: ConstraintType,
    ) -> Box<dyn Constraint> {
        match ct {
            ConstraintType::Generic { ids } => Box::new(self.generate_generic(rand, ids)),
            ConstraintType::Cardinality { ids } => Box::new(self.generate_cardinality(rand, ids)),
            ConstraintType::ThreeAtom { ids } => Box::new(self.generate_three_atom(rand, ids)),
        }
    }

    fn generate_generic(
        &self,
        rand: &mut StdRng,
        ids: Vec<AtomIdent>,
    ) -> ArbitraryLengthConstraint {
        todo!()
    }

    fn generate_cardinality(
        &self,
        rand: &mut StdRng,
        ids: Vec<AtomIdent>,
    ) -> CardinalityConstraint {
        let len_ids = ids.len();
        let degree = rand.random_range(1..len_ids) as i64;
        CardinalityConstraint::new(ids, Comparison::GrEqual, degree)
    }

    pub fn generate_three_atom(&self, rand: &mut StdRng, ids: [AtomIdent; 3]) -> ThreeAtomConstraint {
        let comparison = Comparison::GrEqual;
        let start_val = -self.settings.coeff_range;
        let end_val = self.settings.coeff_range;
        let mut atoms = vec![];
        let mut min_coeff = i64::MAX;
        let mut max_coeff = i64::MIN;
        let mut min_sum_coeff = 0;
        for id in &ids {
            let coeff =
                ConstraintGenerator::sample_without_zero_from_range(rand, start_val, end_val);
            atoms.push(id.to_atom(coeff));

            let coeff = coeff as i64;

            if coeff < 0 {
                min_sum_coeff += coeff;
            }
            if coeff > max_coeff {
                max_coeff = coeff;
            }
            if coeff < min_coeff {
                min_coeff = coeff
            }
        }
        let distance = (max_coeff - min_coeff).abs() as f64;
        let range_start =
            min_sum_coeff + (self.settings.lower_bound_degree * distance).floor() as i64;
        let range_end = min_coeff + (self.settings.upper_bound_degree * distance).ceil() as i64;
        let range_start = min_coeff.max(range_start);
        // this ensures that we have always a single atom, which can already fulfill the equation
        let range_end = max_coeff.min(range_end);
        assert!(
            range_start <= range_end,
            "Range start should be smaller than / equal to range end"
        );
        let atoms = atoms.try_into().expect("Unreachable");
        ThreeAtomConstraint::new(
            atoms,
            comparison,
            rand.random_range(range_start..=range_end),
        )
    }

    fn sample_without_zero_from_range(rand: &mut StdRng, start_val: i32, end_val: i32) -> i32 {
        loop {
            let rand_sample = rand.random_range(start_val..=end_val);
            if rand_sample != 0 {
                return rand_sample;
            }
        }
    }
}
