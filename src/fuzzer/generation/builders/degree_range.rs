use crate::fuzzer::generation::generator_settings::DegreeRatio;

/// Finds the lower and upper bound values in a list of coefficients
///
/// Given n coeffs and a fractional lower and upper bound, determine the minimum
/// and maximum value of a range
pub(super) fn compute_degree_range(coeffs: &[i32], degree_ratio: DegreeRatio) -> (i64, i64) {
    assert!(!coeffs.is_empty());
    let sum_negative: i64 = coeffs.iter().filter(|&&c| c < 0).map(|&c| c as i64).sum();
    let sum_positive: i64 = coeffs.iter().filter(|&&c| c > 0).map(|&c| c as i64).sum();
    let distance = (sum_positive - sum_negative).abs() as f64;
    let (lower_bound_degree, upper_bound_degree) = degree_ratio.get_bounds();
    let range_start = sum_negative + (lower_bound_degree * distance).floor() as i64;
    let range_end = sum_negative + (upper_bound_degree * distance).ceil() as i64;

    assert!(
        range_start <= range_end,
        "Range start should be smaller than / equal to range end"
    );
    (range_start, range_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_bounds() {
        let degree_ratio = DegreeRatio::new(0.25, 0.5);
        let coeffs = vec![-4, -2, 6, 8];
        let (start, end) = compute_degree_range(&coeffs, degree_ratio);
        assert!(!(start..=end).contains(&-2));
        assert!((start..=end).contains(&-1));

        assert!((start..=end).contains(&4));
        assert!(!(start..=end).contains(&5));
    }
}
