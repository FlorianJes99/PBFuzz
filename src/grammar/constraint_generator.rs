use itertools::Itertools;
use rand::{Rng, rngs::StdRng, seq::index};
use std::i64;

use crate::grammar::{
    constraint::{CardinalityConstraint, CostraintWithUnsatAssignments, ThreeAtomConstraint},
    domain::{AtomIdent, Comparison},
};

#[derive(Debug)]
pub struct ConstraintGenerator {
    rand: StdRng,
    settings: ConstraintSettings,
}

impl ConstraintGenerator {
    pub fn new(rand: StdRng, settings: ConstraintSettings) -> Self {
        Self { rand, settings }
    }
}

#[derive(Debug)]
pub struct ConstraintSettings {
    max_vars: u32,
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
    pub fn new(
        max_vars: u32,
        max_unsat_for_single_constraint: u32,
        coeff_range: i32,
        lower_bound_degree: f64,
        upper_bound_degree: f64,
    ) -> Self {
        Self {
            max_vars,
            max_unsat_for_single_constraint,
            coeff_range,
            lower_bound_degree,
            upper_bound_degree,
        }
    }
}

impl ConstraintGenerator {
    pub fn generate_random(&mut self) -> Box<dyn CostraintWithUnsatAssignments> {
        if self.rand.random_bool(0.5) {
            let mut c = self.generate_three_atom();
            // assert we always return a constraint which fullfils max unsat assignments requirement
            while c.amount_unsat_assignments()
                > self.settings.max_unsat_for_single_constraint.into()
            {
                c = self.generate_three_atom();
            }
            Box::new(c)
        } else {
            let amount_atoms = self.rand.random_range(5..=20);
            let c = self.generate_cardinality(amount_atoms);
            Box::new(c)
        }
    }

    pub fn generate_cardinality(&mut self, length: u32) -> CardinalityConstraint {
        let ids = self.get_n_ids_sorted(
            length
                .try_into()
                .expect("Failure while converting u32 to usize"),
        );
        assert!(ids.len() as u32 == length);
        let atoms = ids.iter().map(|id| id.to_atom(1)).collect();
        let degree = self
            .rand
            .random_range(1..=length)
            .try_into()
            .expect("Length of constraint should always fit into i64");
        CardinalityConstraint::new(atoms, Comparison::GrEqual, degree)
            .expect("Builder should ensure correct constraint initalisation")
    }

    pub fn generate_three_atom(&mut self) -> ThreeAtomConstraint {
        let ids = self.get_three_ids_sorted();
        let comparison = Comparison::GrEqual;
        let start_val = -self.settings.coeff_range;
        let end_val = self.settings.coeff_range;
        let mut atoms = vec![];
        let mut min_coeff = i64::MAX;
        let mut max_coeff = i64::MIN;
        let mut min_sum_coeff = 0;
        for id in &ids {
            let coeff = self.sample_without_zero_from_range(start_val, end_val);
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
            self.rand.random_range(range_start..=range_end),
        )
    }

    fn sample_without_zero_from_range(&mut self, start_val: i32, end_val: i32) -> i32 {
        loop {
            let rand_sample = self.rand.random_range(start_val..=end_val);
            if rand_sample != 0 {
                return rand_sample;
            }
        }
    }

    /// Generate n ids to use for creation of atoms in a single constraint
    fn get_n_ids_sorted(&mut self, n: usize) -> Vec<AtomIdent> {
        index::sample(&mut self.rand, self.settings.max_vars as usize, n)
            .iter()
            .map(|x| {
                // add +1 because we want ids from 1..=len and sample are from 0..len
                (x + 1)
                    .try_into()
                    .expect("Error while converting usize to u32 for atom identifier")
            })
            .map(|i| AtomIdent::new(i))
            .sorted()
            .collect()
    }

    /// Methods specifically design for ThreeAtomConstraints
    fn get_three_ids_sorted(&mut self) -> [AtomIdent; 3] {
        let mut ids: [AtomIdent; 3] =
            index::sample_array(&mut self.rand, self.settings.max_vars as usize)
                .expect("Invalid config for sampling. Constraint length > max_vars")
                // add +1 because we want ids from 1..=len and sample are from 0..len
                .map(|x| x as u32 + 1)
                .map(|x| AtomIdent::new(x));
        ids.sort();
        ids
    }
}
