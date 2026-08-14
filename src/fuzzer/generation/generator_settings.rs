use std::fmt::Display;

use serde::{Deserialize, Serialize};

/// All Settings needed for generation of constraints
#[derive(Debug)]
pub struct ConstraintSettings {
    pub max_vars: u32,
    pub max_unsat_for_single_constraint: u32,
    pub amount_literals_per_constraint: u32,
    pub coeff_range: CoeffRange,
    pub degree_ratio: DegreeRatio,
    /// Specifies the weights of each constraint type.
    ///
    /// According to these weights, the constraints are proportionally sampled
    /// E.g.:: (3,1,1,1) will lead to three_literal constraint taking up 50% of the samples,
    /// while the other types will be equally distributed.
    pub constraint_distribution: ConstraintDistribution,
}

#[derive(Debug)]
pub struct CoeffRange {
    /// Minimal value a coefficient can have
    lower: i32,
    /// Maximum value a coefficient can have
    upper: i32,
}

impl CoeffRange {
    pub fn new(lower: i32, upper: i32) -> Self {
        Self { lower, upper }
    }

    pub fn get_bounds(&self) -> (i32, i32) {
        (self.lower, self.upper)
    }
}

#[derive(Debug)]
pub struct DegreeRatio {
    /// Given the sampled literals in a constraint, the lower bound specifies the smallest possible
    /// fraction of the given range
    /// E.g.: If possible assignments yield a range of 0..10 and `lower_bound_degree = 0.3`, then
    ///       lowest possible degree (RHS) can be 3
    lower: f64,
    /// Given the sampled literals in a constraint, the upper bound specifies the highest possible
    /// fraction of the given range
    /// E.g.: If possible assignments yield a range of 0..10 and `upper_bound_degree = 0.6`, then
    ///       highest possible degree (RHS) can be 6
    upper: f64,
}

impl DegreeRatio {
    pub fn new(lower: f64, upper: f64) -> Self {
        Self { lower, upper }
    }

    pub fn get_bounds(&self) -> (f64, f64) {
        (self.lower, self.upper)
    }
}

/// Constraint distributions models the relative occurence of each constraint type in the entire grammar.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct ConstraintDistribution {
    pub three_literal_weight: u32,
    pub cardinality_weight: u32,
    pub arbitrary_weight: u32,
    pub exactly_one_weight: u32,
    pub doubling_pairs_weight: u32,
}

impl ConstraintDistribution {
    pub fn new(
        three_literal_weight: u32,
        cardinality_weight: u32,
        arbitrary_weight: u32,
        exactly_one_weight: u32,
        doubling_pairs_weight: u32,
    ) -> Self {
        Self {
            three_literal_weight,
            cardinality_weight,
            arbitrary_weight,
            exactly_one_weight,
            doubling_pairs_weight,
        }
    }
    pub fn get_weights(&self) -> [u32; 5] {
        [
            self.three_literal_weight,
            self.cardinality_weight,
            self.arbitrary_weight,
            self.exactly_one_weight,
            self.doubling_pairs_weight,
        ]
    }
}

impl Display for ConstraintDistribution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total = self.three_literal_weight + self.cardinality_weight + self.arbitrary_weight;
        let [
            three_literal,
            cardinality,
            arbitrary,
            exactly_one,
            doubling_pairs,
        ]: [f32; 5] = self
            .get_weights()
            .map(|part| (part as f32 / total as f32) * 100.0);
        write!(
            f,
            "Constraint Proportions: Three Literal ({:.2}%), Cardinality ({:.2}%), Arbitrary ({:.2}%), Ecactly One ({:.2}%), Doubling Pairs ({:.2}%)",
            three_literal, cardinality, arbitrary, exactly_one, doubling_pairs
        )
    }
}

impl ConstraintSettings {
    pub fn new(
        max_vars: u32,
        max_unsat_for_single_constraint: u32,
        amount_literals_per_constraint: u32,
        coeff_range: CoeffRange,
        degree_ratio: DegreeRatio,
        constraint_distribution: ConstraintDistribution,
    ) -> Self {
        Self {
            max_vars,
            max_unsat_for_single_constraint,
            amount_literals_per_constraint,
            coeff_range,
            degree_ratio,
            constraint_distribution,
        }
    }
}
