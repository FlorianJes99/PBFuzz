use crate::fuzzer::generation::generator_settings::CoeffRange;
use crate::fuzzer::generation::generator_settings::ConstraintDistribution;
use crate::fuzzer::generation::generator_settings::ConstraintSettings;
use crate::fuzzer::generation::generator_settings::DegreeRatio;
use crate::fuzzer::helpers::OFFSET;
use std::io;
use std::path::Path;

use std::fmt::Display;

use serde::Deserialize;
use serde::Serialize;

use crate::fuzzer::grammar::PBKind;

#[derive(Deserialize, Serialize, Copy, Clone)]
pub struct PbConfigFile {
    pub grammar_kind: PBKind,
    pub generation_method: Option<GenerationMethod>,
    pub random: GeneratorSettings,
    pub from_solution: GeneratorSettings,
}

impl PbConfigFile {
    pub fn load_from_file(file: &Path) -> io::Result<Self> {
        let data = std::fs::read_to_string(file)?;
        toml::from_str(&data).map_err(io::Error::other)
    }

    pub fn generate_settings(self) -> PbSetting {
        let PbConfigFile {
            grammar_kind,
            generation_method,
            random,
            from_solution,
        } = self;
        let generation_method = generation_method.unwrap_or_else(|| {
            if rand::random_bool(0.5) {
                GenerationMethod::Random
            } else {
                GenerationMethod::FromSolution
            }
        });
        match generation_method {
            GenerationMethod::Random => PbSetting::new(grammar_kind, generation_method, random),
            GenerationMethod::FromSolution => {
                PbSetting::new(grammar_kind, generation_method, from_solution)
            }
        }
    }
}

impl Display for PbConfigFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let PbConfigFile {
            grammar_kind,
            generation_method,
            random,
            from_solution,
        } = self;
        writeln!(f, "{:<OFFSET$}: {}", "Mode", grammar_kind)?;
        if let Some(method) = generation_method {
            writeln!(f, "{:<OFFSET$}: {}", "Constraint generation method", method)?;
        }
        writeln!(f)?;
        writeln!(f, "Settings used for random constraint generation:")?;
        writeln!(f, "{}", random)?;
        writeln!(f)?;
        writeln!(
            f,
            "Settings used for constraint generation from existing solution:"
        )?;
        writeln!(f, "{}", from_solution)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PbSetting {
    /// which type of grammar we want to generate: PBS | PBO
    grammar_kind: PBKind,
    generation_method: GenerationMethod,
    generator_settings: GeneratorSettings,
}

impl PbSetting {
    pub fn new(
        grammar_kind: PBKind,
        generation_method: GenerationMethod,
        generator_settings: GeneratorSettings,
    ) -> Self {
        Self {
            grammar_kind,
            generation_method,
            generator_settings,
        }
    }
    pub fn get_generator_settings(&self) -> GeneratorSettings {
        self.generator_settings
    }

    pub fn get_problem_kind(&self) -> &PBKind {
        &self.grammar_kind
    }

    pub fn get_generation_method(&self) -> &GenerationMethod {
        &self.generation_method
    }
}

/// This struct holds all possible configuration parameters for the generation of a random grammar
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct GeneratorSettings {
    /// maximal amount of different variables used in the grammar
    pub max_vars: u32,
    /// different methods to determine when to stop generation of constraints
    cancel_condition: CancelCondition,
    /// How many literals a single constraint should have
    pub amount_literals_per_constraint: u32,
    max_unsat_for_single_constraint: u32,
    /// The possible values a coefficient of a literal can be
    /// E.g.: if `coeff_range = 8`, then values can range from -8..8 except 0
    pub coeff_lower_bound: i32,
    pub coeff_upper_bound: i32,
    /// Given the sampled literals in a constraint, the lower bound specifies the smallest possible
    /// fraction of the given range
    /// E.g.: If possible assignments yield a range of 0..10 and `lower_bound_degree = 0.3`, then
    ///       lowest possible degree (RHS) can be 3
    pub lower_bound_degree: f64,
    /// Given the sampled literals in a constraint, the upper bound specifies the highest possible
    /// fraction of the given range
    /// E.g.: If possible assignments yield a range of 0..10 and `upper_bound_degree = 0.6`, then
    ///       highest possible degree (RHS) can be 6
    pub upper_bound_degree: f64,
    constraint_distribution: ConstraintDistribution,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum CancelCondition {
    CountUnsat { max_unsat_assignments: u64 },
    CountConstraints { max_constraints: u64 },
}

impl Display for CancelCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CancelCondition::CountUnsat {
                max_unsat_assignments,
            } => write!(
                f,
                "{:<OFFSET$}: {}",
                "Max unsat assignments", max_unsat_assignments
            ),
            CancelCondition::CountConstraints { max_constraints } => {
                write!(f, "{:<OFFSET$}: {}", "Max constraints", max_constraints)
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum GenerationMethod {
    #[default]
    Random,
    FromSolution,
}

impl Display for GenerationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenerationMethod::Random => write!(f, "Random"),
            GenerationMethod::FromSolution => write!(f, "From sampled solution"),
        }
    }
}

impl From<GeneratorSettings> for ConstraintSettings {
    fn from(val: GeneratorSettings) -> Self {
        let coeff_range = CoeffRange::new(val.coeff_lower_bound, val.coeff_upper_bound);
        let degree_ratio = DegreeRatio::new(val.lower_bound_degree, val.upper_bound_degree);
        ConstraintSettings::new(
            val.max_vars,
            val.max_unsat_for_single_constraint,
            val.amount_literals_per_constraint,
            coeff_range,
            degree_ratio,
            val.constraint_distribution,
        )
    }
}

impl GeneratorSettings {
    pub fn get_cancel_condition(&self) -> &CancelCondition {
        &self.cancel_condition
    }
}

impl Display for GeneratorSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let GeneratorSettings {
            max_vars,
            cancel_condition,
            amount_literals_per_constraint,
            max_unsat_for_single_constraint,
            coeff_lower_bound,
            coeff_upper_bound,
            lower_bound_degree,
            upper_bound_degree,
            constraint_distribution,
        } = self;
        writeln!(f, "{:<OFFSET$}: {}", "Max used vars", max_vars)?;
        writeln!(f, "{}", cancel_condition)?;
        writeln!(
            f,
            "{:<OFFSET$}: {}",
            "Literals per constraint", amount_literals_per_constraint
        )?;
        writeln!(
            f,
            "{:<OFFSET$}: {}",
            "Max unsat assignments for a single constraint", max_unsat_for_single_constraint
        )?;
        writeln!(
            f,
            "{:<OFFSET$}: {}..{}",
            "Coefficient range", coeff_lower_bound, coeff_upper_bound
        )?;
        writeln!(
            f,
            "{:<OFFSET$}: {}",
            "Lower bound for RHS of constraint", lower_bound_degree
        )?;
        writeln!(
            f,
            "{:<OFFSET$}: {}",
            "Upper bound for RHS of constraint", upper_bound_degree
        )?;
        write!(
            f,
            "{:<OFFSET$}: {}",
            "Constraint distribution", constraint_distribution
        )
    }
}

/// Builder to create Settings for the fuzzer
///
/// Defaults:
/// `coeff_range: 3`
/// `amount_literals_per_constraint: 3`
/// `max_unsat_for_single_constraint: 5`
/// `lower_bound_degree: 0.3`
/// `upper_bound_degree: 0.5`
/// max_unsat_assignments: max_vars * 2.4
#[derive(Debug)]
pub struct GeneratorSettingsBuilder {
    max_vars: u32,
    max_unsat_for_single_constraint: u32,
    cancel_condition: CancelCondition,
    amount_literals_per_constraint: u32,
    coeff_lower_bound: i32,
    coeff_upper_bound: i32,
    lower_bound_degree: f64,
    upper_bound_degree: f64,
    constraint_distribution: ConstraintDistribution,
}

impl Default for GeneratorSettingsBuilder {
    fn default() -> Self {
        let cancel_condition = CancelCondition::CountConstraints {
            max_constraints: 150,
        };
        GeneratorSettingsBuilder::new(100)
            .max_unsat_for_single_constraint(3)
            .cancel_condition(cancel_condition)
            .coeff_lower_bound(-200)
            .coeff_upper_bound(200)
    }
}

impl GeneratorSettingsBuilder {
    pub fn new(max_vars: u32) -> Self {
        let coeff_lower_bound = -3;
        let coeff_upper_bound = 3;
        let amount_literals_per_constraint = 30;
        let max_unsat_for_single_constraint = 5;
        let lower_bound_degree = 0.3;
        let upper_bound_degree = 0.5;
        let cancel_condition = CancelCondition::CountUnsat {
            max_unsat_assignments: (max_vars as f32 * 2.4).floor() as u64,
        };
        let constraint_distribution = ConstraintDistribution::new(2, 1, 1, 1, 1);
        Self {
            max_vars,
            max_unsat_for_single_constraint,
            cancel_condition,
            amount_literals_per_constraint,
            coeff_lower_bound,
            coeff_upper_bound,
            lower_bound_degree,
            upper_bound_degree,
            constraint_distribution,
        }
    }

    pub fn from_settings(settings: GeneratorSettings) -> Self {
        GeneratorSettingsBuilder {
            max_vars: settings.max_vars,
            max_unsat_for_single_constraint: settings.max_unsat_for_single_constraint,
            cancel_condition: settings.cancel_condition,
            amount_literals_per_constraint: settings.amount_literals_per_constraint,
            coeff_lower_bound: settings.coeff_lower_bound,
            coeff_upper_bound: settings.coeff_upper_bound,
            lower_bound_degree: settings.lower_bound_degree,
            upper_bound_degree: settings.upper_bound_degree,
            constraint_distribution: settings.constraint_distribution,
        }
    }

    pub fn max_unsat_for_single_constraint(mut self, max: u32) -> Self {
        self.max_unsat_for_single_constraint = max;
        self
    }

    pub fn cancel_condition(mut self, cancel_condition: CancelCondition) -> Self {
        self.cancel_condition = cancel_condition;
        self
    }

    pub fn coeff_lower_bound(mut self, lower_bound: i32) -> Self {
        self.coeff_lower_bound = lower_bound;
        self
    }

    pub fn coeff_upper_bound(mut self, upper_bound: i32) -> Self {
        self.coeff_upper_bound = upper_bound;
        self
    }

    /// this method panics if lower > upper!
    pub fn set_bounds_degree(mut self, lower: f64, upper: f64) -> Self {
        assert!(lower <= upper, "{lower} expected to be <= {upper}");
        self.lower_bound_degree = lower;
        self.upper_bound_degree = upper;
        self
    }

    pub fn set_constraint_distribution(
        mut self,
        constraint_distribution: ConstraintDistribution,
    ) -> Self {
        self.constraint_distribution = constraint_distribution;
        self
    }

    pub fn build(self) -> GeneratorSettings {
        GeneratorSettings {
            max_vars: self.max_vars,
            max_unsat_for_single_constraint: self.max_unsat_for_single_constraint,
            cancel_condition: self.cancel_condition,
            amount_literals_per_constraint: self.amount_literals_per_constraint,
            coeff_lower_bound: self.coeff_lower_bound,
            coeff_upper_bound: self.coeff_upper_bound,
            lower_bound_degree: self.lower_bound_degree,
            upper_bound_degree: self.upper_bound_degree,
            constraint_distribution: self.constraint_distribution,
        }
    }
}
