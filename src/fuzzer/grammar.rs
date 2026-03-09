use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Display;
use std::io::Result;
use std::io::Write;
use std::time::Duration;
use std::time::Instant;

use itertools::Itertools;
use log::info;
use serde::Deserialize;
use serde::Serialize;

use crate::fuzzer::constraint::Constraint;
use crate::fuzzer::constraint::ConstraintKind;
use crate::fuzzer::domain::AtomIdent;
use crate::fuzzer::domain::MinFunction;
use crate::fuzzer::domain::MinObjectiveValue;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, clap::ValueEnum, Default)]
pub enum PBKind {
    /// pseudo boolean solving
    #[default]
    PBS,
    /// pseudo boolean optimization
    PBO,
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
    seed: u64,
    amount_vars: u32,
    amount_constraints: u32,
    amount_unsat_assignments: Option<u64>,
    constraints: Vec<Box<dyn Constraint>>,
    min_constraint: Option<MinFunction>,
    generation_time: Duration,
}

impl PBGrammar {
    pub fn new(
        seed: u64,
        amount_unsat_assignments: Option<u64>,
        amount_distinct_vars: u32,
        constraints: Vec<Box<dyn Constraint>>,
        min_constraint: Option<MinFunction>,
        generation_time: Duration,
    ) -> Self {
        let amount_constraints = constraints
            .len()
            .try_into()
            .expect("We should never generate more constraints than 2**32");
        Self {
            seed,
            amount_vars: amount_distinct_vars,
            amount_constraints,
            amount_unsat_assignments,
            constraints,
            min_constraint,
            generation_time,
        }
    }
    pub fn produce<W: Write>(&mut self, writer: &mut W) -> Result<Duration> {
        let start = Instant::now();
        info!(
            "Generated random grammar PBS grammar with {} vars and {} constraints.",
            self.amount_vars, self.amount_constraints
        );
        let amount_equal_constraints = 0; //TODO: implement this as soon as constraints can be
        //equal with RHS
        writeln!(
            writer,
            "* #variable= {} #constraint= {} #equal= {} intsize= 32",
            self.amount_vars, self.amount_constraints, amount_equal_constraints
        )?;
        writeln!(writer, "* Used seed: {}", self.seed)?;
        if let Some(n) = self.amount_unsat_assignments {
            writeln!(writer, "* UNSAT assignments: {}", n)?;
        }
        writeln!(writer, "{}", self.get_statistics())?;

        // write min constraint, if present
        if let Some(c) = &self.min_constraint {
            writeln!(writer, "{}", c)?;
        }

        // write every constraint into the buffer
        for constraint in &self.constraints {
            writeln!(writer, "{}", constraint)?;
        }
        writer.flush()?;
        Ok(start.elapsed())
    }

    pub fn get_statistics(&self) -> GrammarStatistics {
        let mut occurences_per_var: HashMap<AtomIdent, u32> = HashMap::new();
        for c in &self.constraints {
            for a in c.atoms() {
                occurences_per_var
                    .entry(a.atom_id)
                    .and_modify(|v| *v += 1)
                    .or_insert(1);
            }
        }
        let mut max: Option<OccurenceWithId> = Option::None;
        let mut min: Option<OccurenceWithId> = Option::None;
        let mut sum = 0;
        for (id, occurences) in occurences_per_var.iter() {
            if max.is_none() {
                max = Some(OccurenceWithId::new(id.clone(), *occurences));
            } else if max.is_some_and(|f| *occurences > f.count) {
                max = Some(OccurenceWithId::new(id.clone(), *occurences));
            }
            if min.is_none() {
                min = Some(OccurenceWithId::new(id.clone(), *occurences));
            } else if min.is_some_and(|f| *occurences < f.count) {
                min = Some(OccurenceWithId::new(id.clone(), *occurences));
            }
            sum += occurences;
        }
        let average = sum as f32 / occurences_per_var.len() as f32;
        let amount_constraints_by_kind = self.constraints.iter().counts_by(|v| v.kind());
        GrammarStatistics {
            amount_constraints: self.amount_constraints,
            amount_constraints_by_kind,
            amount_vars: self.amount_vars,
            total_var_occurences: sum,
            max_occurence: max.expect("There should always be a var which occurs the most"),
            min_occurence: min.expect("There should always be a var which occurs the least"),
            average_occurence: average,
            generation_time: self.generation_time,
        }
    }

    pub fn verify_solution(&self, true_vars: &HashSet<AtomIdent>) -> bool {
        self.constraints.iter().all(|c| c.verify(&true_vars))
    }

    pub fn verify_min_constraint(
        &self,
        true_vars: &HashSet<AtomIdent>,
        objective: &MinObjectiveValue,
    ) -> bool {
        self.min_constraint
            .as_ref()
            .map(|c| -> MinObjectiveValue {
                c.get_atoms()
                    .iter()
                    .filter(|a| true_vars.contains(&a.atom_id))
                    .map(|a| a.coeff)
                    .sum::<i32>()
                    .into()
            })
            .map(|min| &min == objective)
            .unwrap_or(false)
    }
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
        writeln!(f, "* Amount different used variables: {}", self.amount_vars)?;
        writeln!(f, "* Amount constraints: {}", self.amount_constraints)?;
        for (k, v) in &self.amount_constraints_by_kind {
            writeln!(f, "* Amount constraints of type {k}: {v}")?;
        }
        writeln!(
            f,
            "* Total amount of used vars: {}",
            self.total_var_occurences
        )?;
        writeln!(f, "* Max occurence of a var: {}", self.max_occurence)?;
        writeln!(f, "* Min occurence of a var: {}", self.min_occurence)?;
        writeln!(
            f,
            "* Average occurences of vars: {}",
            self.average_occurence
        )?;
        write!(
            f,
            "* Generation time: {}ms",
            self.generation_time.as_millis()
        )
    }
}

#[derive(Clone, Copy)]
struct OccurenceWithId {
    atom_id: AtomIdent,
    count: u32,
}

impl OccurenceWithId {
    fn new(atom_id: AtomIdent, count: u32) -> Self {
        Self { atom_id, count }
    }
}

impl Display for OccurenceWithId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.count, self.atom_id)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_statistics() {
        use crate::fuzzer::constraint::CostraintWithUnsatAssignments;
        use crate::fuzzer::constraint::ThreeAtomConstraint;
        use crate::fuzzer::domain::{Atom, Comparison};
        let atoms1 = [Atom::new(1, 2), Atom::new(2, 5), Atom::new(4, 1)];
        let atoms2 = [Atom::new(2, 2), Atom::new(3, 5), Atom::new(4, 1)];
        let atoms3 = [Atom::new(1, 2), Atom::new(3, 5), Atom::new(4, 1)];
        let constraints = vec![
            ThreeAtomConstraint::new(atoms1, Comparison::GrEqual, 5),
            ThreeAtomConstraint::new(atoms2, Comparison::GrEqual, 5),
            ThreeAtomConstraint::new(atoms3, Comparison::GrEqual, 5),
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
        let grammar = PBGrammar::new(
            42,
            unsat_assignments,
            4,
            constraints,
            Option::None,
            Duration::from_secs(0),
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
