use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Display;
use std::io;
use std::time::Duration;
use std::time::Instant;

use itertools::Itertools;
use log::info;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use crate::fuzzer::app_info::MetaData;
use crate::fuzzer::constraints::constraint::Constraint;
use crate::fuzzer::constraints::constraint::ConstraintKind;
use crate::fuzzer::constraints::errors::ConstraintViolation;
use crate::fuzzer::domain::Comparison;
use crate::fuzzer::domain::LiteralIdent;
use crate::fuzzer::domain::MinObjectiveValue;
use crate::fuzzer::domain::Objective;
use crate::fuzzer::domain::SolutionModel;
use crate::fuzzer::helpers::ms;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, clap::ValueEnum, Default)]
pub enum PBKind {
    /// pseudo boolean solving
    PBS,
    /// pseudo boolean optimization
    #[default]
    PBO,
}

impl PBKind {
    pub fn get_short_str(&self) -> &str {
        match self {
            PBKind::PBS => "PBS",
            PBKind::PBO => "PBO",
        }
    }
}

impl Display for PBKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                PBKind::PBS => "Pseudo Boolean Solving",
                PBKind::PBO => "Pseudo Boolean Optimization",
            }
        )
    }
}

pub struct PBGrammar {
    program_info: MetaData,
    seed: u64,
    amount_vars: u32,
    amount_constraints: u32,
    constraints: Vec<Box<dyn Constraint>>,
    objective: Option<Objective>,
    generation_time: Duration,
    generation_source: GenerationSource,
}

impl PBGrammar {
    pub fn new(
        program_info: MetaData,
        seed: u64,
        amount_distinct_vars: u32,
        constraints: Vec<Box<dyn Constraint>>,
        objective: Option<Objective>,
        generation_time: Duration,
        generation_source: GenerationSource,
    ) -> Self {
        let amount_constraints = constraints
            .len()
            .try_into()
            .expect("We should never generate more constraints than 2**32");
        Self {
            program_info,
            seed,
            amount_vars: amount_distinct_vars,
            amount_constraints,
            constraints,
            objective,
            generation_time,
            generation_source,
        }
    }
    /// Writes the generated PBGrammer into a writer
    ///
    /// Returns the duration of writing into the buffer
    pub fn produce<W: io::Write>(&self, writer: &mut W) -> io::Result<Duration> {
        let Self {
            program_info,
            seed,
            amount_vars,
            amount_constraints,
            constraints,
            objective: min_constraint,
            generation_time: _, // we print this in the statistics later
            generation_source,
        } = self;
        let start = Instant::now();
        info!(
            "Generated PB grammar with {} vars and {} constraints.",
            amount_vars, amount_constraints
        );
        let amount_equal_constraints = constraints
            .iter()
            .filter(|c| c.comparison() == &Comparison::Equal)
            .count();
        writeln!(
            writer,
            "* #variable= {} #constraint= {} #equal= {} intsize= 32",
            amount_vars, amount_constraints, amount_equal_constraints
        )?;
        let MetaData {
            program_version,
            commit_hash,
        } = program_info;
        writeln!(writer, "* Program version: {program_version}")?;
        writeln!(writer, "* Commit Hash: {commit_hash}")?;
        writeln!(writer, "* Used seed: {}", seed)?;
        match generation_source {
            GenerationSource::Random {
                amount_unsat_assignments,
            } => {
                if let Some(n) = amount_unsat_assignments {
                    writeln!(writer, "* UNSAT assignments: {}", n)?;
                }
            }
            GenerationSource::FromSolution { model, objective } => {
                writeln!(writer, "* Generated solution: {}", model)?;
                if let Some(o) = objective {
                    writeln!(
                        writer,
                        "* Objective Value for sampled Model (FromSolution Mode): {o}"
                    )?
                }
            }
        }
        writeln!(writer, "{}", self.get_statistics())?;

        // write min constraint, if present
        if let Some(c) = min_constraint {
            writeln!(writer, "{}", c)?;
        }

        // write every constraint into the buffer
        for constraint in constraints {
            writeln!(writer, "{}", constraint)?;
        }
        writer.flush()?;
        Ok(start.elapsed())
    }

    pub fn get_statistics(&self) -> GrammarStatistics {
        let occurences_per_var = self
            .constraints
            .iter()
            .flat_map(|c| c.literals())
            .counts_by(|literal| literal.literal_id);
        let max = occurences_per_var
            .iter()
            .max_by(|(id_a, count_a), (id_b, count_b)| count_a.cmp(count_b).then(id_b.cmp(id_a)))
            .map(|(id, occ)| OccurenceWithId::new(*id, *occ as u32));
        let min = occurences_per_var
            .iter()
            .min_by(|(id_a, count_a), (id_b, count_b)| count_a.cmp(count_b).then(id_b.cmp(id_a)))
            .map(|(id, occ)| OccurenceWithId::new(*id, *occ as u32));
        let sum: usize = occurences_per_var.values().sum();
        let average = sum as f32 / occurences_per_var.len() as f32;
        let amount_constraints_by_kind = self.constraints.iter().counts_by(|v| v.kind());
        GrammarStatistics {
            amount_constraints: self.amount_constraints,
            amount_constraints_by_kind,
            amount_vars: self.amount_vars,
            total_var_occurences: sum as u32,
            max_occurence: max.expect("There should always be a var which occurs the most"),
            min_occurence: min.expect("There should always be a var which occurs the least"),
            average_occurence: average,
            generation_time: self.generation_time,
        }
    }

    pub fn verify_solution(&self, true_vars: &SolutionModel) -> Result<(), ConstraintViolation> {
        self.constraints
            .iter()
            .try_for_each(|c| c.verify(true_vars))
    }

    pub fn verify_min_constraint(
        &self,
        true_vars: &HashSet<LiteralIdent>,
        objective: MinObjectiveValue,
    ) -> Result<(), MinObjectiveInvalid> {
        if let GenerationSource::FromSolution {
            model: _,
            objective: objective_from_solution,
        } = &self.generation_source
            && let Some(obj) = objective_from_solution
        {
            info!("Generated objective value from solution: {obj}; found minimum:  {objective}")
        }
        match self.objective.as_ref() {
            Some(mf) => {
                let eval_min = mf
                    .get_literals()
                    .iter()
                    .filter(|a| true_vars.contains(&a.literal_id))
                    .map(|a| a.coeff)
                    .sum::<i32>()
                    .into();
                if eval_min == objective {
                    Ok(())
                } else {
                    Err(MinObjectiveInvalid {
                        evaluated: eval_min,
                        expected: objective,
                    })
                }
            }
            None => {
                unreachable!("Minimize objective should always exist, when running in pbo mode")
            }
        }
    }
}

#[derive(Debug, Error)]
#[error("Minimum objective differs: evaluated from model: {evaluated}, solver found: {expected}")]
pub struct MinObjectiveInvalid {
    evaluated: MinObjectiveValue,
    expected: MinObjectiveValue,
}

pub enum GenerationSource {
    Random {
        amount_unsat_assignments: Option<u64>,
    },
    FromSolution {
        model: SolutionModel,
        objective: Option<MinObjectiveValue>,
    },
}

/// Container for various statistics about the generated grammar
pub struct GrammarStatistics {
    amount_constraints: u32,
    amount_constraints_by_kind: HashMap<ConstraintKind, usize>,
    amount_vars: u32,
    total_var_occurences: u32,
    max_occurence: OccurenceWithId,
    min_occurence: OccurenceWithId,
    average_occurence: f32,
    generation_time: Duration,
}

impl GrammarStatistics {
    pub fn get_generation_time(&self) -> &Duration {
        &self.generation_time
    }
}

impl Display for GrammarStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let GrammarStatistics {
            amount_constraints,
            amount_constraints_by_kind,
            amount_vars,
            total_var_occurences,
            max_occurence,
            min_occurence,
            average_occurence,
            generation_time,
        } = self;
        writeln!(f, "* Amount different used variables: {}", amount_vars)?;
        writeln!(f, "* Amount constraints: {}", amount_constraints)?;
        for (k, v) in amount_constraints_by_kind.iter().sorted_by_key(|&(k, _)| k) {
            writeln!(f, "* Amount constraints of type {k}: {v}")?;
        }
        writeln!(f, "* Total amount of used vars: {}", total_var_occurences)?;
        writeln!(f, "* Max occurence of a var: {}", max_occurence)?;
        writeln!(f, "* Min occurence of a var: {}", min_occurence)?;
        writeln!(f, "* Average occurences of vars: {}", average_occurence)?;
        write!(f, "* Generation time: {:.3}ms", ms(generation_time))
    }
}

#[derive(Clone, Copy)]
struct OccurenceWithId {
    literal_id: LiteralIdent,
    count: u32,
}

impl OccurenceWithId {
    fn new(literal_id: LiteralIdent, count: u32) -> Self {
        Self { literal_id, count }
    }
}

impl Display for OccurenceWithId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.count, self.literal_id)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fuzzer::constraints::constraint::CostraintWithUnsatAssignments;
    use crate::fuzzer::constraints::three_literals::ThreeLiteralConstraint;

    #[test]
    fn test_statistics() {
        use crate::fuzzer::domain::{Comparison, Literal};
        let literals1 = [Literal::new(1, 2), Literal::new(2, 5), Literal::new(4, 1)];
        let literals2 = [Literal::new(2, 2), Literal::new(3, 5), Literal::new(4, 1)];
        let literals3 = [Literal::new(1, 2), Literal::new(3, 5), Literal::new(4, 1)];
        let constraints = vec![
            ThreeLiteralConstraint::new(literals1, Comparison::GrEqual, 5),
            ThreeLiteralConstraint::new(literals2, Comparison::GrEqual, 5),
            ThreeLiteralConstraint::new(literals3, Comparison::GrEqual, 5),
        ];
        let unsat_assignments = Option::Some(
            constraints
                .iter()
                .map(|c| c.amount_unsat_assignments() as u64)
                .sum(),
        );
        let constraints: Vec<Box<dyn Constraint>> = constraints
            .into_iter()
            .map(|a| Box::new(a) as Box<dyn Constraint>)
            .collect();
        let program_info = MetaData {
            program_version: "1.0.0".into(),
            commit_hash: "unknown".into(),
        };
        let grammar = PBGrammar::new(
            program_info,
            42,
            4,
            constraints,
            Option::None,
            Duration::from_secs(0),
            GenerationSource::Random {
                amount_unsat_assignments: unsat_assignments,
            },
        );
        let stats = grammar.get_statistics();
        assert_eq!(stats.amount_vars, 4);
        assert_eq!(stats.amount_constraints, 3);
        assert_eq!(stats.max_occurence.count, 3);
        assert_eq!(stats.min_occurence.count, 2);
        assert_eq!(stats.total_var_occurences, 9);
        assert!((stats.average_occurence - 2.25).abs() < 0.001);
    }
}
