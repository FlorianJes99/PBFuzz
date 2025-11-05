use itertools::Itertools;
use log::info;
use std::{
    cmp::min,
    io::{self, Write},
};

use crate::domain::*;
use rand::{Rng, SeedableRng, rngs::StdRng, seq::index};

pub trait Grammar {
    fn produce<W: Write>(&mut self, writer: &mut W) -> io::Result<()>;
}

#[derive(Debug)]
struct PBSGrammar {
    constraints: Vec<Constraint>,
    amount_vars: u32,
    amount_constraints: u32,
}

impl PBSGrammar {
    fn new(constraints: Vec<Constraint>) -> Self {
        let amount_constraints = constraints
            .len()
            .try_into()
            .expect("We should never generate more constraints than 2**32");
        let amount_vars = constraints
            .iter()
            .flat_map(|c| c.atoms())
            .map(|a| a.var_id)
            .unique()
            .count()
            .try_into()
            .expect("We should never try to create programs with more than 2**32 vars");
        Self {
            constraints,
            amount_vars,
            amount_constraints,
        }
    }
}

#[derive(Debug)]
pub struct PBSGenerator {
    seed: u64,
    max_vars: u32,
    rand: StdRng,
}

impl PBSGenerator {
    pub fn new(seed: u64, max_vars: u32) -> Self {
        Self {
            seed,
            max_vars,
            rand: StdRng::seed_from_u64(seed),
        }
    }

    fn random_grammar(&mut self) -> PBSGrammar {
        let mut unsat_assignments = 0;
        let mut constraints = vec![];
        let max = (2_u32.pow(self.max_vars) as f32 * 0.7).floor() as u32;
        loop {
            let c = self.random_constraint();
            unsat_assignments += c.unsat_count();
            constraints.push(c);
            if unsat_assignments > max {
                let n = constraints.len();
                info!(
                    "Reached maximum unsatisfiable assignments: {}",
                    unsat_assignments
                );
                info!("Generated {} constraints!", n);
                break;
            }
        }
        PBSGrammar::new(constraints)
    }

    fn random_constraint(&mut self) -> Constraint {
        // range for coefficient of each atom
        let coeff_range = 10;
        // generate always 3 atoms if possible
        let min_atoms = min(3, self.max_vars);
        let amount_atoms = min_atoms;
        // at least one atom should be in a constraint
        let sample = index::sample(
            &mut self.rand,
            self.max_vars as usize,
            amount_atoms as usize,
        );
        let atoms: Vec<Atom> = sample
            .iter()
            .sorted()
            // add indices + 1, because sample method samples from range (0..length)
            .map(|n| n + 1)
            .map(|n| {
                Atom::new(
                    n.try_into()
                        .expect("We should always be able to cast to u32 for atom var ids"),
                    self.rand.random_range(-coeff_range..=coeff_range),
                )
            })
            .collect();

        let comparison = Comparison::GrEqual;
        self.create_from_atoms(atoms, comparison)
    }

    fn create_from_atoms(&mut self, atoms: Vec<Atom>, comparison: Comparison) -> Constraint {
        let min_coeff: i64 = atoms
            .iter()
            .filter(|atom| atom.coeff < 0)
            .map(|atom| atom.coeff as i64)
            .sum();

        let max_coeff: i64 = atoms
            .iter()
            .filter(|atom| atom.coeff > 0)
            .map(|atom| atom.coeff as i64)
            .sum();
        let distance = (max_coeff - min_coeff).abs();
        let shrink = distance / 4;
        let range_start = min_coeff + shrink;
        let range_end = max_coeff - shrink;
        assert!(
            range_start <= range_end,
            "Range start should be smaller than / equal to range end"
        );
        let constraint = Constraint::new(
            atoms,
            comparison,
            self.rand.random_range(range_start..=range_end),
        );
        debug_assert!(
            constraint.is_solvable(),
            "A constraint on each own should be solvable"
        );
        constraint
    }
}

impl Grammar for PBSGenerator {
    fn produce<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        let grammar = self.random_grammar();
        info!(
            "Generated random grammar PBS grammar with {} vars and {} constraints.",
            grammar.amount_vars, grammar.amount_constraints
        );
        writeln!(
            writer,
            "* #variable= {} #constraints= {}",
            grammar.amount_vars, grammar.amount_constraints
        )?;
        writeln!(writer, "* Used seed: {}", self.seed)?;

        // write every constraint into the buffer
        for constraint in grammar.constraints.iter() {
            writeln!(writer, "{}", constraint)?;
        }
        Ok(())
    }
}
