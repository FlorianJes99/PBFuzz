use itertools::Itertools;
use log::info;
use std::io::{self, Write};

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
            .map(|a| a.literal_id)
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

    fn amount_unsat_assignments(&self) -> u32 {
        self.constraints
            .iter()
            .map(|c| c.unsat_count_three_atoms())
            .sum()
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
        let amount_constraints = (self.max_vars as f32 * 4.2).floor() as u32;
        // let max = (2_u32.pow(self.max_vars) as f32 * 0.3).floor() as u32;
        info!("Generating amount of constraints: {}", amount_constraints);
        let constraints: Vec<Constraint> = (0..amount_constraints)
            .map(|_| self.random_constraint())
            .collect();
        info!("Generated {} constraints!", amount_constraints);
        PBSGrammar::new(constraints)
    }

    fn random_constraint(&mut self) -> Constraint {
        // range for coefficient of each atom
        let coeff_range = 10;
        // generate always 3 atoms if possible
        let amount_atoms = 3;
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
                    self.sample_without_zero_from_range(-coeff_range, coeff_range),
                )
            })
            .collect();

        let comparison = Comparison::GrEqual;
        self.create_from_atoms(atoms, comparison)
    }

    fn sample_without_zero_from_range(&mut self, start_val: i32, end_val: i32) -> i32 {
        loop {
            let rand_sample = self.rand.random_range(start_val..=end_val);
            if rand_sample != 0 {
                return rand_sample;
            }
        }
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
        let distance = (max_coeff - min_coeff).abs() as f64;
        // place the possible RHS values inside the central 20% of possible ranges
        let range_start = min_coeff + (0.4 * distance).floor() as i64;
        let range_end = min_coeff + (0.6 * distance).ceil() as i64;
        assert!(
            range_start <= range_end,
            "Range start should be smaller than / equal to range end"
        );
        let constraint = Constraint::new(
            atoms,
            comparison,
            self.rand.random_range(range_start..=range_end),
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
        let amount_equal_constraints = 0; //TODO: implement this as soon as constraints can be
        //equal with RHS
        writeln!(
            writer,
            "* #variable= {} #constraint= {} #equal= {} intsize= 32",
            grammar.amount_vars, grammar.amount_constraints, amount_equal_constraints
        )?;
        writeln!(writer, "* Used seed: {}", self.seed)?;
        writeln!(
            writer,
            "* UNSAT assignments: {}",
            grammar.amount_unsat_assignments()
        )?;

        // write every constraint into the buffer
        for constraint in grammar.constraints.iter() {
            writeln!(writer, "{}", constraint)?;
        }
        Ok(())
    }
}
