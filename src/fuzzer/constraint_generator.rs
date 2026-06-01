use itertools::Itertools;
use rand::{
    Rng,
    distr::{Distribution, weighted::WeightedIndex},
    rngs::StdRng,
    seq::{IndexedRandom, index},
};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

use crate::fuzzer::{
    constraint::{
        ArbitraryLengthConstraint, CardinalityConstraint, Constraint, ConstraintKind,
        CostraintWithUnsatAssignments, ExactlyOneConstraint, ThreeLiteralConstraint,
    },
    domain::{Comparison, Literal, LiteralIdent, MinFunction},
};

#[derive(Debug)]
pub struct ConstraintGenerator {
    rand: StdRng,
    dist: WeightedIndex<i32>,
    settings: ConstraintSettings,
}

impl ConstraintGenerator {
    pub fn new(rand: StdRng, settings: ConstraintSettings) -> Self {
        let dist = WeightedIndex::new(settings.constraint_distribution.get_weights())
            .expect("We should only create valid distributions, when sampling constraint types");
        Self {
            rand,
            dist,
            settings,
        }
    }

    fn sample_constraint_kind(&mut self) -> ConstraintKind {
        let index = self.dist.sample(&mut self.rand);
        match index {
            0 => ConstraintKind::ThreeLiteral,
            1 => ConstraintKind::Cardinality,
            2 => ConstraintKind::ArbitraryLength,
            3 => ConstraintKind::ExactlyOne,
            _ => unreachable!(),
        }
    }

    pub fn generate_random_constraint_with_unsat_count(
        &mut self,
    ) -> Box<dyn CostraintWithUnsatAssignments> {
        if self.rand.random_bool(0.5) {
            let mut c = self.generate_three_literal();
            // Assert we always return a constraint which fulfills max unsat assignments requirement
            while c.amount_unsat_assignments()
                > <u32 as Into<u128>>::into(self.settings.max_unsat_for_single_constraint)
            {
                c = self.generate_three_literal();
            }
            Box::new(c)
        } else {
            let amount_literals = self.rand.random_range(5..=20);
            let c = self.generate_cardinality(amount_literals);
            Box::new(c)
        }
    }
    pub fn generate_random_constraint(&mut self) -> Box<dyn Constraint> {
        let kind = self.sample_constraint_kind();
        match kind {
            ConstraintKind::ThreeLiteral => {
                let mut c = self.generate_three_literal();
                // Assert we always return a constraint which fulfills max unsat assignments requirement
                while c.amount_unsat_assignments()
                    > std::convert::Into::<u128>::into(
                        self.settings.max_unsat_for_single_constraint,
                    )
                {
                    c = self.generate_three_literal();
                }
                Box::new(c)
            }
            ConstraintKind::Cardinality => {
                let constraint_len =
                    if self.settings.max_vars <= self.settings.amount_literals_per_constraint {
                        (self.settings.max_vars as f32 * 0.5).floor() as u32
                    } else {
                        self.settings.amount_literals_per_constraint
                    };
                let c = self.generate_cardinality(constraint_len);
                Box::new(c)
            }
            ConstraintKind::ArbitraryLength => {
                let constraint_len =
                    if self.settings.max_vars <= self.settings.amount_literals_per_constraint {
                        (self.settings.max_vars as f32 * 0.5).floor() as u32
                    } else {
                        self.settings.amount_literals_per_constraint
                    };
                let c = self.generate_generic(constraint_len);
                Box::new(c)
            }
            ConstraintKind::ExactlyOne => {
                // we want to have at max 3 literals in the exactly one constraint for now
                let constraint_len = 3.min((self.settings.max_vars as f32 * 0.5).floor() as u32);
                let c = self.generate_exactly_one_constraint(constraint_len);
                Box::new(c)
            }
        }
    }

    pub fn generate_generic(&mut self, length: u32) -> ArbitraryLengthConstraint {
        assert!(length != 0, "Length of constraint needs to be > 0");
        let ids = self.get_n_ids_sorted(length);
        let start_val = self.settings.coeff_lower_bound;
        let end_val = self.settings.coeff_upper_bound;
        let mut literals = vec![];
        let mut min_coeff = i64::MAX;
        let mut max_coeff = i64::MIN;
        let mut min_sum_coeff = 0;
        for id in &ids {
            let coeff = self.sample_without_zero_from_range(start_val, end_val);
            literals.push(id.to_literal(coeff));

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
        // This ensures that we have always a single literal, which can already fulfill the equation
        let range_end = max_coeff.min(range_end);
        assert!(
            range_start <= range_end,
            "Range start should be smaller than / equal to range end"
        );

        let comparison = Comparison::GrEqual;
        let degree = self.rand.random_range(range_start..=range_end);
        ArbitraryLengthConstraint::new(literals, comparison, degree)
    }

    pub fn generate_cardinality(&mut self, length: u32) -> CardinalityConstraint {
        assert!(length != 0, "Length of constraint needs to be > 0");
        let ids = self.get_n_ids_sorted(length);
        assert!(ids.len() as u32 == length);
        let mut literals = vec![];
        let mut lower_max: i32 = 0;
        let mut upper_max: i32 = 0;
        for id in &ids {
            if self.rand.random_bool(0.5) {
                literals.push(id.to_literal(1));
                upper_max += 1;
            } else {
                literals.push(id.to_literal(-1));
                lower_max -= 1;
            }
        }
        assert!(literals.len() as u32 == length);
        let distance = upper_max - lower_max;
        let lower_bound =
            lower_max + (self.settings.lower_bound_degree * distance as f64).ceil() as i32;
        let upper_bound =
            lower_max + (self.settings.upper_bound_degree * distance as f64).floor() as i32;
        let degree = self.rand.random_range(lower_bound..upper_bound).into();
        CardinalityConstraint::new(literals, Comparison::GrEqual, degree)
            .expect("Builder should ensure correct constraint initalisation")
    }

    pub fn generate_three_literal(&mut self) -> ThreeLiteralConstraint {
        let ids = self.get_three_ids_sorted();
        let comparison = Comparison::GrEqual;
        let start_val = self.settings.coeff_lower_bound;
        let end_val = self.settings.coeff_upper_bound;
        let mut literals = vec![];
        let mut min_coeff = i64::MAX;
        let mut max_coeff = i64::MIN;
        let mut min_sum_coeff = 0;
        for id in &ids {
            let coeff = self.sample_without_zero_from_range(start_val, end_val);
            literals.push(id.to_literal(coeff));

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
        // This ensures that we have always a single literal, which can already fulfill the equation
        let range_end = max_coeff.min(range_end);
        assert!(
            range_start <= range_end,
            "Range start should be smaller than / equal to range end"
        );
        let literals = literals.try_into().expect("Unreachable");
        ThreeLiteralConstraint::new(
            literals,
            comparison,
            self.rand.random_range(range_start..=range_end),
        )
    }

    pub fn generate_exactly_one_constraint(&mut self, length: u32) -> ExactlyOneConstraint {
        assert!(length != 0, "Length of constraint needs to be > 0");
        let literals: Vec<Literal> = self
            .get_n_ids_sorted(length)
            .iter()
            .map(|id| id.to_literal(1))
            .collect();

        assert!(literals.len() as u32 == length);
        ExactlyOneConstraint::new(literals, Comparison::Equal, 1)
            .expect("Builder should ensure correct constraint initalisation")
    }

    pub fn generate_min_function(
        &mut self,
        possible_vars: &[LiteralIdent],
        amount: usize,
    ) -> MinFunction {
        let literals = possible_vars
            .choose_multiple(&mut self.rand, amount)
            .sorted()
            .map(|id| id.to_literal(self.sample_without_zero_from_range(-3, 3)))
            .collect();
        MinFunction::new(literals)
    }

    fn sample_without_zero_from_range(&mut self, start_val: i32, end_val: i32) -> i32 {
        loop {
            let rand_sample = self.rand.random_range(start_val..=end_val);
            if rand_sample != 0 {
                return rand_sample;
            }
        }
    }

    /// Generate n ids to use for creation of literals in a single constraint
    fn get_n_ids_sorted(&mut self, n: u32) -> Vec<LiteralIdent> {
        index::sample(&mut self.rand, self.settings.max_vars as usize, n as usize)
            .iter()
            .map(|x| {
                // Add +1 because we want ids from 1..=len and sample are from 0..len
                (x + 1)
                    .try_into()
                    .expect("Error while converting usize to u32 for literal identifier")
            })
            .map(LiteralIdent::new)
            .sorted()
            .collect()
    }

    /// Methods specifically design for ThreeLiteralConstraints
    fn get_three_ids_sorted(&mut self) -> [LiteralIdent; 3] {
        let mut ids: [LiteralIdent; 3] =
            index::sample_array(&mut self.rand, self.settings.max_vars as usize)
                .expect("Invalid config for sampling. Constraint length > max_vars")
                // Add +1 because we want ids from 1..=len and sample are from 0..len
                .map(|x| x as u32 + 1)
                .map(LiteralIdent::new);
        ids.sort();
        ids
    }
}

#[derive(Debug)]
pub struct ConstraintSettings {
    max_vars: u32,
    max_unsat_for_single_constraint: u32,
    amount_literals_per_constraint: u32,
    /// Minimal value a coefficient can have
    coeff_lower_bound: i32,
    /// Maximum value a coefficient can have
    coeff_upper_bound: i32,
    /// Given the sampled literals in a constraint, the lower bound specifies the smallest possible
    /// fraction of the given range
    /// E.g.: If possible assignments yield a range of 0..10 and `lower_bound_degree = 0.3`, then
    ///       lowest possible degree (RHS) can be 3
    lower_bound_degree: f64,
    /// Given the sampled literals in a constraint, the upper bound specifies the highest possible
    /// fraction of the given range
    /// E.g.: If possible assignments yield a range of 0..10 and `upper_bound_degree = 0.6`, then
    ///       highest possible degree (RHS) can be 6
    upper_bound_degree: f64,
    /// Specifies the weights of each constraint type.
    ///
    /// According to these weights, the constraints are proportionally sampled
    /// E.g.:: (3,1,1,1) will lead to three_literal constraint taking up 50% of the samples,
    /// while the other types will be equally distributed.
    constraint_distribution: ConstraintDistribution,
}

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
    fn get_weights(&self) -> [i32; 4] {
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
        coeff_lower_bound: i32,
        coeff_upper_bound: i32,
        lower_bound_degree: f64,
        upper_bound_degree: f64,
        constraint_distribution: ConstraintDistribution,
    ) -> Self {
        Self {
            max_vars,
            max_unsat_for_single_constraint,
            amount_literals_per_constraint,
            coeff_lower_bound,
            coeff_upper_bound,
            lower_bound_degree,
            upper_bound_degree,
            constraint_distribution,
        }
    }
}
