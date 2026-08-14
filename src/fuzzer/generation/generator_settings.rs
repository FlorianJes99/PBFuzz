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
    three_literal_weight: i32,
    cardinality_weight: i32,
    arbitrary_weight: i32,
    exactly_one_weight: i32,
}

impl ConstraintDistribution {
    pub fn new(
        three_literal_weight: i32,
        cardinality_weight: i32,
        arbitrary_weight: i32,
        exactly_one_weight: i32,
    ) -> Self {
        Self {
            three_literal_weight,
            cardinality_weight,
            arbitrary_weight,
            exactly_one_weight,
        }
    }
    pub fn get_weights(&self) -> [i32; 4] {
        [
            self.three_literal_weight,
            self.cardinality_weight,
            self.arbitrary_weight,
            self.exactly_one_weight,
        ]
    }
}

impl Display for ConstraintDistribution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total = self.three_literal_weight + self.cardinality_weight + self.arbitrary_weight;
        let percentages: [f32; 4] = self
            .get_weights()
            .map(|part| (part as f32 / total as f32) * 100.0);
        write!(
            f,
            "Constraint Proportions: Three Literal ({:.2}%), Cardinality ({:.2}%), Arbitrary ({:.2}%), Ecactly One ({:.2}%)",
            percentages[0], percentages[1], percentages[2], percentages[3]
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
