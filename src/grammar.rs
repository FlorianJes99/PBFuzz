use itertools::Itertools;
use log::info;
use std::{
    cmp::min,
    fmt::Display,
    io::{self, Write},
};

use rand::{Rng, SeedableRng, rngs::StdRng, seq::index};

pub trait Grammar {
    fn produce<W: Write>(&mut self, writer: &mut W) -> io::Result<()>;
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

    fn unsat_count(&self) -> u32 {
        let n = self.atoms.len();
        assert_eq!(
            n, 3,
            "This method works only for exactly three atoms per constraint"
        );

        let [a1, a2, a3] = &self.atoms[..] else {
            unreachable!();
        };
        let (a1, a2, a3) = (a1.coeff, a2.coeff, a3.coeff);
        let assignments = vec![
            (0, 0, 0),
            (1, 0, 0),
            (0, 1, 0),
            (0, 0, 1),
            (1, 1, 0),
            (1, 0, 1),
            (0, 1, 1),
            (1, 1, 1),
        ];
        assignments
            .iter()
            .map(|(b1, b2, b3)| b1 * a1 + b2 * a2 + b3 * a3)
            .filter(|&sum| !self.satisfied(sum))
            .count()
            .try_into()
            .expect("Should be able to convert at max 8 assignments to a u32 number")
    }

    fn satisfied(&self, value: i32) -> bool {
        match self.comparison {
            Comparison::GrEqual => value as i64 >= self.result_val,
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
            .flat_map(|c| c.atoms.iter())
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

#[test]
fn display_atom() {
    let atom = Atom::new(1, 3);
    assert_eq!(atom.to_string(), "3 x1".to_string());

    let negative_coeff = Atom::new(1, -3);
    assert_eq!(negative_coeff.to_string(), "-3 x1".to_string());
}

#[test]
fn amount_unsat() {
    let atoms = vec![Atom::new(1, 5), Atom::new(3, -1), Atom::new(5, 3)];
    let c = Constraint::new(atoms, Comparison::GrEqual, 4);
    assert_eq!(c.unsat_count(), 4);

    let atoms = vec![Atom::new(1, 5), Atom::new(3, -1), Atom::new(5, 3)];
    let c = Constraint::new(atoms, Comparison::GrEqual, 0);
    assert_eq!(c.unsat_count(), 1)
}

#[test]
fn display_constraint() {
    let atoms: Vec<Atom> = (1..=3).map(|i| Atom::new(i as u32, i * 10)).collect();

    let comp = Comparison::GrEqual;
    let res = 15;

    let constr = Constraint::new(atoms, comp, res);

    assert_eq!(constr.to_string(), "10 x1 20 x2 30 x3 >= 15;")
}
