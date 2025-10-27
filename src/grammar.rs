use itertools::Itertools;
use std::fmt::Display;

use rand::{Rng, SeedableRng, rngs::StdRng};

pub trait Grammar {
    fn produce(&mut self) -> String;
}

#[derive(Debug)]
struct Atom {
    var_name: String,
    coeff: i32,
}

impl Atom {
    fn new(var_name: String, coeff: i32) -> Self {
        Self { var_name, coeff }
    }
}

impl Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sign = match self.coeff.cmp(&0) {
            std::cmp::Ordering::Less => "-",
            std::cmp::Ordering::Equal => "",
            std::cmp::Ordering::Greater => "+",
        };
        write!(f, "{}{} {}", sign, self.coeff.abs(), self.var_name)
    }
}

#[derive(Debug)]
enum Comparison {
    Equal,
    GrEqual,
}

impl Display for Comparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Comparison::Equal => "=",
            Comparison::GrEqual => ">=",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug)]
struct Constraint {
    atoms: Vec<Atom>,
    comparison: Comparison,
    result_val: i32,
}

impl Constraint {
    fn new(atoms: Vec<Atom>, comparison: Comparison, result_val: i32) -> Self {
        Self {
            atoms,
            comparison,
            result_val,
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
}

impl Display for OPBGrammar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let amount_constraint = self.constraints.len();
        let amount_vars = self.constraints.iter()
            .flat_map(|c| c.atoms.iter())
            .map(|a| &a.var_name)
            .unique()
            .count();
        let header = format!("* #variable= {} #constraints= {}", amount_vars, amount_constraint);
        let repr = self.constraints.iter().join("\n");
        write!(f, "{header}\n{repr}")
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
        let constraints = (0..self.amount_constraints)
            .map(|_| self.random_constraint())
            .collect();
        OPBGrammar {
            constraints: constraints,
        }
    }

    fn random_constraint(&mut self) -> Constraint {
        // there should be at max k-1 atoms for k vars
        let amount_atoms = self.rand.random_range(1..=self.max_vars - 1);
        // at least one atom should be in a constraint
        let atoms: Vec<Atom> = (1..=amount_atoms)
            .map(|i| Atom::new(format!("x{}", i), self.rand.random()))
            .collect();

        let comparison = if self.rand.random_bool(0.5) {
            Comparison::Equal
        } else {
            Comparison::GrEqual
        };

        let min_coeff = atoms
            .iter()
            .map(|atom| atom.coeff)
            .min()
            .expect("There should always be at least one atom in a constraint");

        let max_coeff = atoms.iter().map(|atom| atom.coeff).sum();
        Constraint::new(
            atoms,
            comparison,
            self.rand.random_range(min_coeff..=max_coeff),
        )
    }
}

impl Grammar for PBSGenerator {
    fn produce(&mut self) -> String {
        let grammar = self.random_grammar();
        grammar.to_string()
    }
}

#[test]
fn display_atom() {
    let atom = Atom::new("x1".to_string(), 3);
    assert_eq!(atom.to_string(), "+3 x1".to_string());

    let negative_coeff = Atom::new("x1".to_string(), -3);
    assert_eq!(negative_coeff.to_string(), "-3 x1".to_string());
}

#[test]
fn display_constraint() {
    let atoms: Vec<Atom> = (1..=3)
        .map(|i| Atom::new(format!("x{i}"), i * 10))
        .collect();

    let comp = Comparison::Equal;
    let res = 15;

    let constr = Constraint::new(atoms, comp, res);

    assert_eq!(constr.to_string(), "+10 x1 +20 x2 +30 x3 = 15")
}
