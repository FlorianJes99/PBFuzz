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
    pub random: GeneratorSettings<RandomBuildersConfig>,
    pub from_solution: GeneratorSettings<FromSolutionBuildersConfig>,
}

/// This struct holds all possible configuration parameters for the generation of a random grammar
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct GeneratorSettings<B: Display> {
    /// maximal amount of different variables used in the grammar
    pub max_vars: u32,
    pub amount_lits_min_term: u32,
    /// different methods to determine when to stop generation of constraints
    cancel_condition: CancelCondition,
    builders_config: B,
}

#[derive(Deserialize, Serialize, Copy, Clone)]
pub struct BuilderEntry<C: Display> {
    pub weight: u32,
    #[serde(flatten)]
    pub settings: C,
}

impl<C: Display> Display for BuilderEntry<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { weight, settings } = self;
        writeln!(f, "{:<OFFSET$}: {weight}", "Weight")?;
        write!(f, "{settings}")
    }
}

#[derive(Deserialize, Serialize, Copy, Clone)]
pub struct RandomBuildersConfig {
    pub three_literal: BuilderEntry<ThreeLiteralConfig>,
    pub cardinality: BuilderEntry<CardinalityConfig>,
    pub generic: BuilderEntry<GenericConfig>,
    pub exactly_one: BuilderEntry<ExactlyOneConfig>,
    pub doubling_pairs: BuilderEntry<DoublingPairsConfig>,
}

impl Display for RandomBuildersConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            three_literal,
            cardinality,
            generic,
            exactly_one,
            doubling_pairs,
        } = self;
        writeln!(f, "Three literal constraint:")?;
        writeln!(f, "{three_literal}")?;
        writeln!(f)?;
        writeln!(f, "Cardinality constraint:")?;
        writeln!(f, "{cardinality}")?;
        writeln!(f)?;
        writeln!(f, "Arbitrary length constraint:")?;
        writeln!(f, "{generic}")?;
        writeln!(f)?;
        writeln!(f, "Exactly one constraint:")?;
        writeln!(f, "{exactly_one}")?;
        writeln!(f)?;
        writeln!(f, "Doubling pairs constraint:")?;
        write!(f, "{doubling_pairs}")
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct FromSolutionBuildersConfig {
    pub generic: BuilderEntry<GenericConfig>,
    pub doubling_pairs: BuilderEntry<DoublingPairsConfig>,
}

impl Display for FromSolutionBuildersConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            generic,
            doubling_pairs,
        } = self;
        writeln!(f, "Generic constraint:")?;
        writeln!(f, "{generic}")?;
        writeln!(f)?;
        writeln!(f, "Doubling pairs constraint:")?;
        write!(f, "{doubling_pairs}")
    }
}

#[derive(Deserialize, Serialize, Copy, Clone)]
pub struct ThreeLiteralConfig {
    pub coeff_lower: i32,
    pub coeff_upper: i32,
    pub degree_lower: f64,
    pub degree_upper: f64,
}

impl Display for ThreeLiteralConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            coeff_lower,
            coeff_upper,
            degree_lower,
            degree_upper,
        } = self;
        writeln!(
            f,
            "{:<OFFSET$}: {coeff_lower}..{coeff_upper}",
            "Coefficient range"
        )?;
        write!(
            f,
            "{:<OFFSET$}: {degree_lower}..{degree_upper}",
            "Degree range"
        )
    }
}

#[derive(Deserialize, Serialize, Copy, Clone)]
pub struct GenericConfig {
    pub amount_lits: u32,
    pub coeff_lower: i32,
    pub coeff_upper: i32,
    pub degree_lower: f64,
    pub degree_upper: f64,
}

impl Display for GenericConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            amount_lits,
            coeff_lower,
            coeff_upper,
            degree_lower,
            degree_upper,
        } = self;
        writeln!(f, "{:<OFFSET$}: {amount_lits}", "Literals per constraint")?;
        writeln!(
            f,
            "{:<OFFSET$}: {coeff_lower}..{coeff_upper}",
            "Coefficient range"
        )?;
        write!(
            f,
            "{:<OFFSET$}: {degree_lower}..{degree_upper}",
            "Degree range"
        )
    }
}

#[derive(Deserialize, Serialize, Copy, Clone)]
pub struct CardinalityConfig {
    pub amount_lits: u32,
    pub degree_lower: f64,
    pub degree_upper: f64,
}

impl Display for CardinalityConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            amount_lits,
            degree_lower,
            degree_upper,
        } = self;
        writeln!(f, "{:<OFFSET$}: {amount_lits}", "Literals per constraint")?;
        write!(
            f,
            "{:<OFFSET$}: {degree_lower}..{degree_upper}",
            "Degree range"
        )
    }
}

#[derive(Deserialize, Serialize, Copy, Clone)]
pub struct ExactlyOneConfig {
    pub amount_lits: u32,
}

impl Display for ExactlyOneConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { amount_lits } = self;
        write!(f, "{:<OFFSET$}: {amount_lits}", "Literals per constraint")
    }
}

#[derive(Deserialize, Serialize, Copy, Clone)]
pub struct DoublingPairsConfig {
    pub amount_pairs: u32,
}

impl Display for DoublingPairsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { amount_pairs } = self;
        let amount_lits = amount_pairs * 2;
        write!(f, "{:<OFFSET$}: {amount_lits}", "Literals per constraint")
    }
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
            GenerationMethod::Random => {
                PbSetting::new(grammar_kind, GenerationMethodWithSettings::Random(random))
            }
            GenerationMethod::FromSolution => PbSetting::new(
                grammar_kind,
                GenerationMethodWithSettings::FromSolution(from_solution),
            ),
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
        write!(f, "{}", from_solution)
    }
}

#[derive(Clone, Copy)]
pub enum GenerationMethodWithSettings {
    Random(GeneratorSettings<RandomBuildersConfig>),
    FromSolution(GeneratorSettings<FromSolutionBuildersConfig>),
}

impl GenerationMethodWithSettings {
    pub fn get_max_vars(&self) -> u32 {
        match self {
            GenerationMethodWithSettings::Random(generator_settings) => generator_settings.max_vars,
            GenerationMethodWithSettings::FromSolution(generator_settings) => {
                generator_settings.max_vars
            }
        }
    }

    pub fn get_literals_min_objective(&self) -> u32 {
        match self {
            GenerationMethodWithSettings::Random(generator_settings) => {
                generator_settings.amount_lits_min_term
            }
            GenerationMethodWithSettings::FromSolution(generator_settings) => {
                generator_settings.amount_lits_min_term
            }
        }
    }

    pub fn get_cancel_condition(&self) -> &CancelCondition {
        match self {
            GenerationMethodWithSettings::Random(generator_settings) => {
                &generator_settings.cancel_condition
            }
            GenerationMethodWithSettings::FromSolution(generator_settings) => {
                &generator_settings.cancel_condition
            }
        }
    }
}

#[derive(Copy, Clone)]
pub struct PbSetting {
    /// which type of grammar we want to generate: PBS | PBO
    grammar_kind: PBKind,
    generation_method: GenerationMethodWithSettings,
}

impl PbSetting {
    pub fn new(grammar_kind: PBKind, generation_method: GenerationMethodWithSettings) -> Self {
        Self {
            grammar_kind,
            generation_method,
        }
    }

    pub fn get_max_vars(&self) -> u32 {
        self.generation_method.get_max_vars()
    }
    pub fn get_generator_settings(&self) -> GenerationMethodWithSettings {
        self.generation_method
    }

    pub fn get_problem_kind(&self) -> &PBKind {
        &self.grammar_kind
    }

    pub fn get_generation_method(&self) -> &GenerationMethod {
        match self.generation_method {
            GenerationMethodWithSettings::Random(_) => &GenerationMethod::Random,
            GenerationMethodWithSettings::FromSolution(_) => &GenerationMethod::FromSolution,
        }
    }
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

impl<B: Display + Clone + Copy> GeneratorSettings<B> {
    pub fn get_cancel_condition(&self) -> &CancelCondition {
        &self.cancel_condition
    }

    pub fn get_builder_config(&self) -> &B {
        &self.builders_config
    }
}

impl<B: Display> Display for GeneratorSettings<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let GeneratorSettings {
            max_vars,
            amount_lits_min_term,
            cancel_condition,
            builders_config,
        } = self;
        writeln!(f, "{:<OFFSET$}: {}", "Max used vars", max_vars)?;
        writeln!(
            f,
            "{:<OFFSET$}: {}",
            "Amount literals in minimization term", amount_lits_min_term
        )?;
        writeln!(f, "{}", cancel_condition)?;
        writeln!(f)?;
        write!(f, "{builders_config}")
    }
}
