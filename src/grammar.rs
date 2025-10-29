use itertools::Itertools;
use log::info;
use std::{cmp::max, fmt::Display};

use rand::{Rng, SeedableRng, rngs::StdRng, seq::index};

pub trait Grammar {
    fn produce(&mut self) -> String;
}

#[derive(Debug)]
struct Atom {
    var_id: u32,
    coeff: i32,
}

impl Atom {
    fn new(var_id: u32, coeff: i32) -> Self {
        Self { var_id, coeff }
    }
}

impl Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} x{}", self.coeff, self.var_id)
    }
}

#[derive(Debug)]
enum Comparison {
    //TODO:  Equal,
    GrEqual,
}

impl Display for Comparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            // Comparison::Equal => "=",
            Comparison::GrEqual => ">=",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug)]
struct Constraint {
    atoms: Vec<Atom>,
    comparison: Comparison,
    result_val: i64,
}

impl Constraint {
    fn new(atoms: Vec<Atom>, comparison: Comparison, result_val: i64) -> Self {
        assert!(
            !atoms.is_empty(),
            "Constraint needs at least one atom to be valid"
        );
        Self {
            atoms,
            comparison,
            result_val,
        }
    }

    fn is_solvable(&self) -> bool {
        match self.comparison {
            Comparison::GrEqual => {
                self.atoms
                    .iter()
                    .filter(|atom| atom.coeff > 0)
                    .map(|atom| atom.coeff as i64)
                    .sum::<i64>()
                    >= self.result_val
            }
        }
    }
}

impl Display for Constraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let atoms: String = self.atoms.iter().map(|x| x.to_string()).join(" ");

        write!(f, "{} {} {};", atoms, self.comparison, self.result_val)
    }
}

#[derive(Debug)]
struct OPBGrammar {
    constraints: Vec<Constraint>,
    amount_vars: usize,
    amount_constraints: usize,
}

impl OPBGrammar {
    fn new(constraints: Vec<Constraint>) -> Self {
        let amount_constraints = constraints.len();
        let amount_vars = constraints
            .iter()
            .flat_map(|c| c.atoms.iter())
            .map(|a| a.var_id)
            .unique()
            .count();
        Self {
            constraints,
            amount_vars,
            amount_constraints,
        }
    }
}

impl Display for OPBGrammar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // write the header into the buffer
        writeln!(
            f,
            "* #variable= {} #constraints= {}",
            self.amount_vars, self.amount_constraints
        )?;

        // write every constraint into the buffer
        for constraint in self.constraints.iter() {
            writeln!(f, "{}", constraint)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PBSGenerator {
    max_vars: u32,
    amount_constraints: u32,
    rand: StdRng,
}

impl PBSGenerator {
    pub fn new(seed: u64, max_vars: u32, amount_constraints: u32) -> Self {
        Self {
            max_vars,
            amount_constraints,
            rand: StdRng::seed_from_u64(seed),
        }
    }

    fn random_grammar(&mut self) -> OPBGrammar {
        let constraints: Vec<Constraint> = (0..self.amount_constraints)
            .map(|_| self.random_constraint())
            .collect();
        assert_eq!(
            constraints.len(),
            self.amount_constraints as usize,
            "Generated amount constraints must be the same like specified amount of constraints"
        );
        OPBGrammar::new(constraints)
    }

    fn random_constraint(&mut self) -> Constraint {
        // range for coefficient of each atom
        let coeff_range = 10;
        // generate at least two atoms if possible
        let min_atoms = max(self.max_vars / 2, 1);
        // there should be at max k-1 atoms for k vars
        let amount_atoms = self.rand.random_range(min_atoms..=self.max_vars - 1);
        // at least one atom should be in a constraint
        let sample = index::sample(
            &mut self.rand,
            self.max_vars as usize,
            amount_atoms as usize,
        );
        let atoms: Vec<Atom> = sample
            .iter()
            // add indices + 1, because sample method samples from range (0..length)
            .map(|n| n + 1)
            .map(|n| {
                Atom::new(
                    n.try_into()
                        .expect("We should always be able to cast to u32 for atom var ids"),
                    self.rand.random_range(-coeff_range..=coeff_range),
                )
            })
            .sorted_by(|a, b| a.var_id.cmp(&b.var_id))
            .collect();

        let comparison = Comparison::GrEqual;
        let min_coeff = atoms
            .iter()
            .filter(|atom| atom.coeff < 0)
            .map(|atom| atom.coeff as i64)
            .sum();

        let max_coeff: i64 = atoms
            .iter()
            .filter(|atom| atom.coeff > 0)
            .map(|atom| atom.coeff as i64)
            .sum();
        let constraint = Constraint::new(
            atoms,
            comparison,
            self.rand.random_range(min_coeff..=max_coeff),
        );
        assert!(
            constraint.is_solvable(),
            "A constraint on each own should be solvable"
        );
        constraint
    }
}

impl Grammar for PBSGenerator {
    fn produce(&mut self) -> String {
        let grammar = self.random_grammar();
        info!(
            "Generated random grammar PBS grammar with {} vars and {} constraints.",
            grammar.amount_vars, grammar.amount_constraints
        );
        grammar.to_string()
    }
}

#[test]
fn display_atom() {
    let atom = Atom::new(1, 3);
    assert_eq!(atom.to_string(), "3 x1".to_string());

    let negative_coeff = Atom::new(1, -3);
    assert_eq!(negative_coeff.to_string(), "-3 x1".to_string());
}

#[test]
fn display_constraint() {
    let atoms: Vec<Atom> = (1..=3).map(|i| Atom::new(i as u32, i * 10)).collect();

    let comp = Comparison::GrEqual;
    let res = 15;

    let constr = Constraint::new(atoms, comp, res);

    assert_eq!(constr.to_string(), "10 x1 20 x2 30 x3 >= 15;")
}
