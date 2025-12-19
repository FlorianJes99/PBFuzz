use itertools::Itertools;
use rand::{Rng, rngs::StdRng, seq::index};
use std::i64;

use crate::grammar::{
    constraint::{
        ArbitraryLengthConstraint, CardinalityConstraint, Constraint, ThreeAtomConstraint,
    },
    domain::{AtomIdent, Comparison},
};

#[derive(Debug)]
pub enum ConstraintKind {
    Generic { constraint_length: u32 },
    Cardinality { constraint_length: u32 },
    ThreeAtom,
}

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
    pub fn generate_constraint(&mut self, ct: ConstraintKind) -> Box<dyn Constraint> {
        match ct {
            ConstraintKind::Generic {constraint_length: n } => Box::new(self.generate_generic(n)),
            ConstraintKind::Cardinality { constraint_length: n } => Box::new(self.generate_cardinality(n)),
            ConstraintKind::ThreeAtom => Box::new(self.generate_three_atom()),
        }
    }

    fn generate_generic(&mut self, length: u32) -> ArbitraryLengthConstraint {
        todo!()
    }

    fn generate_cardinality(&mut self, length: u32) -> CardinalityConstraint {
        let ids = self.get_n_ids_sorted(length.try_into().expect("Failure while converting u32 to usize"));
        let atoms = ids.iter().map(|id| id.to_atom(1)).collect();
        let degree = self.rand.random_range(1..length) as i64;
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

    fn get_n_ids_sorted(&mut self, n: usize) -> Vec<AtomIdent> {
        index::sample(&mut self.rand, self.settings.max_vars as usize, n)
            .iter()
            .map(|i| {
                AtomIdent::new(
                    i.try_into()
                        .expect("Error while converting usize to u32 for atom identifier"),
                )
            })
            .sorted()
            .collect()
    }

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
