//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Pearson Correlation & Multi-Lag Spillover Analytics
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};

/// Represents an identified lead-lag cross-asset relationship between two tickers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpilloverResult {
    /// Leading / Base asset ticker symbol
    pub ticker_a: String,
    /// Lagging / Target asset ticker symbol
    pub ticker_b: String,
    /// Optimal lead-lag offset in hours (positive => A leads B; negative => B leads A)
    pub lag_hours: i64,
    /// Pearson cross-correlation coefficient at the optimal lag (-1.0 to 1.0)
    pub correlation: f64,
    /// Number of paired observation buckets evaluated
    pub num_observations: usize,
}

/// Compute the standard Pearson product-moment correlation coefficient between two equal-length slices.
/// Returns 0.0 if vectors have zero variance, mismatched length, or fewer than 2 elements.
pub fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    if n != y.len() || n < 2 {
        return 0.0;
    }

    let n_f = n as f64;
    let sum_x: f64 = x.iter().sum();
    let sum_y: f64 = y.iter().sum();
    let mean_x = sum_x / n_f;
    let mean_y = sum_y / n_f;

    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;

    for i in 0..n {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    if var_x <= 1e-12 || var_y <= 1e-12 {
        return 0.0;
    }

    let r = cov / (var_x.sqrt() * var_y.sqrt());
    r.clamp(-1.0, 1.0)
}

/// Computes cross-correlation across all integer lags in `[-max_lag, +max_lag]`
/// and identifies the lag with the highest absolute correlation coefficient.
///
/// Returns `(optimal_lag_hours, max_correlation, num_observations)`.
pub fn compute_cross_correlation(
    series_a: &[f64],
    series_b: &[f64],
    max_lag: i64,
) -> (i64, f64, usize) {
    let n = series_a.len();
    if n != series_b.len() || n < 3 {
        return (0, 0.0, 0);
    }

    let mut best_lag = 0;
    let mut best_corr = 0.0;
    let mut best_abs_corr = 0.0;
    let mut best_obs = n;

    let safe_max_lag = max_lag.abs().min((n as i64 / 2).max(1));

    for lag in -safe_max_lag..=safe_max_lag {
        let (slice_a, slice_b, obs) = if lag > 0 {
            let k = lag as usize;
            if k >= n {
                continue;
            }
            (&series_a[..n - k], &series_b[k..], n - k)
        } else if lag < 0 {
            let k = lag.unsigned_abs() as usize;
            if k >= n {
                continue;
            }
            (&series_a[k..], &series_b[..n - k], n - k)
        } else {
            (series_a, series_b, n)
        };

        if obs < 2 {
            continue;
        }

        let r = pearson_correlation(slice_a, slice_b);
        let abs_r = r.abs();

        if abs_r > best_abs_corr {
            best_abs_corr = abs_r;
            best_corr = r;
            best_lag = lag;
            best_obs = obs;
        }
    }

    (best_lag, best_corr, best_obs)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_pearson_identical_series() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let r = pearson_correlation(&x, &y);
        assert!((r - 1.0).abs() < 1e-6, "Expected r=1.0, got {}", r);
    }

    #[test]
    fn test_pearson_inverted_series() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![5.0, 4.0, 3.0, 2.0, 1.0];
        let r = pearson_correlation(&x, &y);
        assert!((r - (-1.0)).abs() < 1e-6, "Expected r=-1.0, got {}", r);
    }

    #[test]
    fn test_pearson_zero_variance() {
        let x = vec![2.0, 2.0, 2.0, 2.0];
        let y = vec![1.0, 3.0, 2.0, 4.0];
        let r = pearson_correlation(&x, &y);
        assert_eq!(r, 0.0);
    }

    #[test]
    fn test_pearson_mismatched_lengths() {
        let x = vec![1.0, 2.0];
        let y = vec![1.0, 2.0, 3.0];
        assert_eq!(pearson_correlation(&x, &y), 0.0);
    }

    #[test]
    fn test_cross_correlation_positive_lag_lead() {
        // Ticker A moves first, Ticker B follows 1 hour later
        let a = vec![0.1, 0.8, -0.4, 0.6, -0.2, 0.9, -0.7, 0.3, 0.5, -0.1];
        // B is A shifted by 1 hour (lag +1)
        let b = vec![0.0, 0.1, 0.8, -0.4, 0.6, -0.2, 0.9, -0.7, 0.3, 0.5];

        let (lag, corr, obs) = compute_cross_correlation(&a, &b, 3);
        assert_eq!(lag, 1, "Expected Ticker A to lead Ticker B by lag=1");
        assert!(
            (corr - 1.0).abs() < 1e-5,
            "Expected correlation ~ 1.0, got {}",
            corr
        );
        assert_eq!(obs, 9);
    }

    #[test]
    fn test_cross_correlation_negative_lag_lagging() {
        // Ticker B moves first, Ticker A follows 2 hours later (A lags B by 2 => lag = -2)
        let b = vec![
            0.2, 0.9, -0.5, 0.7, -0.1, 0.8, -0.6, 0.4, 0.6, -0.2, 0.3, 0.1,
        ];
        let a = vec![
            0.0, 0.0, 0.2, 0.9, -0.5, 0.7, -0.1, 0.8, -0.6, 0.4, 0.6, -0.2,
        ];

        let (lag, corr, _obs) = compute_cross_correlation(&a, &b, 3);
        assert_eq!(lag, -2, "Expected lag=-2 (B leads A by 2 hours)");
        assert!(
            (corr - 1.0).abs() < 1e-5,
            "Expected correlation ~ 1.0, got {}",
            corr
        );
    }

    #[test]
    fn test_cross_correlation_contemporaneous() {
        let a = vec![0.5, -0.3, 0.8, -0.6, 0.2, 0.4];
        let b = vec![0.55, -0.28, 0.82, -0.59, 0.19, 0.41];

        let (lag, corr, obs) = compute_cross_correlation(&a, &b, 2);
        assert_eq!(lag, 0);
        assert!(corr > 0.98);
        assert_eq!(obs, 6);
    }
}
