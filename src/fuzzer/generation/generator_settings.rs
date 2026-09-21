#[derive(Debug, Clone, Copy)]
pub struct CoeffRange {
    /// Minimal value a coefficient can have
    lower: i32,
    /// Maximum value a coefficient can have
    upper: i32,
}

impl CoeffRange {
    pub fn new(lower: i32, upper: i32) -> Self {
        Self { lower, upper }
    }

    pub fn get_bounds(&self) -> (i32, i32) {
        (self.lower, self.upper)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DegreeRatio {
    /// Given the sampled literals in a constraint, the lower bound specifies the smallest possible
    /// fraction of the given range
    /// E.g.: If possible assignments yield a range of 0..10 and `lower_bound_degree = 0.3`, then
    ///       lowest possible degree (RHS) can be 3
    lower: f64,
    /// Given the sampled literals in a constraint, the upper bound specifies the highest possible
    /// fraction of the given range
    /// E.g.: If possible assignments yield a range of 0..10 and `upper_bound_degree = 0.6`, then
    ///       highest possible degree (RHS) can be 6
    upper: f64,
}

impl DegreeRatio {
    pub fn new(lower: f64, upper: f64) -> Self {
        Self { lower, upper }
    }

    pub fn get_bounds(&self) -> (f64, f64) {
        (self.lower, self.upper)
    }
}
