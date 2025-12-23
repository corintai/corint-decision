//! Score normalization utilities for converting raw scores to canonical 0-1000 range.
//!
//! This module provides industry-standard score normalization methods, particularly
//! the sigmoid/logistic function which is commonly used in risk management systems.

/// Configuration for score normalization
#[derive(Debug, Clone)]
pub struct ScoreNormalizer {
    /// Center point (midpoint score, typically representing 50% risk)
    /// When raw score equals x0, canonical score will be 500
    pub x0: f64,

    /// Slope/sensitivity parameter (larger k = steeper curve)
    /// Typical values: 0.005 - 0.02
    pub k: f64,
}

impl Default for ScoreNormalizer {
    fn default() -> Self {
        Self {
            x0: 500.0,  // Center point at 500
            k: 0.01,    // Moderate slope
        }
    }
}

impl ScoreNormalizer {
    /// Create a new score normalizer with custom parameters
    ///
    /// # Arguments
    /// * `x0` - Center point (50% risk score)
    /// * `k` - Slope/sensitivity (higher = steeper curve)
    ///
    /// # Example
    /// ```
    /// use corint_sdk::score::ScoreNormalizer;
    ///
    /// let normalizer = ScoreNormalizer::new(500.0, 0.01);
    /// let canonical = normalizer.normalize(750);
    /// ```
    pub fn new(x0: f64, k: f64) -> Self {
        Self { x0, k }
    }

    /// Normalize raw score to canonical 0-1000 range using sigmoid/logistic function
    ///
    /// Formula: canonical = 1000 / (1 + e^(-k * (raw - x0)))
    ///
    /// This provides a smooth S-curve that:
    /// - Maps unbounded raw scores to 0-1000 range
    /// - Has smooth transitions (no hard cutoffs)
    /// - Is configurable via k and x0 parameters
    /// - Handles extreme values gracefully
    ///
    /// # Arguments
    /// * `raw` - Raw score from rule evaluation
    ///
    /// # Returns
    /// Canonical score in range 0-1000
    pub fn normalize(&self, raw: i32) -> i32 {
        if raw < 0 {
            return 0;
        }

        let raw_f64 = raw as f64;
        let exponent = -self.k * (raw_f64 - self.x0);
        let sigmoid = 1000.0 / (1.0 + exponent.exp());

        // Clamp to ensure we stay in valid range (handles edge cases)
        sigmoid.round().clamp(0.0, 1000.0) as i32
    }

    /// Normalize with explicit parameters (convenience method)
    pub fn normalize_with_params(raw: i32, x0: f64, k: f64) -> i32 {
        Self::new(x0, k).normalize(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_normalizer() {
        let normalizer = ScoreNormalizer::default();

        // Negative scores become 0
        assert_eq!(normalizer.normalize(-100), 0);

        // Center point (x0=500) should give ~500
        let center = normalizer.normalize(500);
        assert!(center >= 495 && center <= 505, "Center score: {}", center);

        // Low scores should be low but not zero
        let low = normalizer.normalize(100);
        assert!(low > 0 && low < 200, "Low score: {}", low);

        // High scores should approach 1000
        let high = normalizer.normalize(1500);
        assert!(high > 900 && high <= 1000, "High score: {}", high);

        // Very high scores should saturate near 1000
        let very_high = normalizer.normalize(5000);
        assert!(very_high >= 990 && very_high <= 1000, "Very high score: {}", very_high);
    }

    #[test]
    fn test_custom_parameters() {
        // Steeper curve (higher k)
        let steep = ScoreNormalizer::new(500.0, 0.02);

        // Gentler curve (lower k)
        let gentle = ScoreNormalizer::new(500.0, 0.005);

        // For same raw score, steeper curve should have more extreme values
        let raw = 700;
        let steep_score = steep.normalize(raw);
        let gentle_score = gentle.normalize(raw);

        // Steep should be closer to the extremes
        assert!(steep_score > gentle_score, "Steep: {}, Gentle: {}", steep_score, gentle_score);
    }

    #[test]
    fn test_score_monotonicity() {
        let normalizer = ScoreNormalizer::default();

        // Scores should increase monotonically
        let mut prev = 0;
        for raw in (0..2000).step_by(100) {
            let canonical = normalizer.normalize(raw);
            assert!(canonical >= prev, "Score should increase monotonically");
            prev = canonical;
        }
    }

    #[test]
    fn test_edge_cases() {
        let normalizer = ScoreNormalizer::default();

        // Zero score should be very low but not zero (due to sigmoid)
        let zero_score = normalizer.normalize(0);
        assert!(zero_score > 0 && zero_score < 50, "Zero score: {}", zero_score);

        // Very large score shouldn't exceed 1000
        assert_eq!(normalizer.normalize(100000), 1000);

        // Negative score
        assert_eq!(normalizer.normalize(-1000), 0);
    }

    #[test]
    fn test_different_center_points() {
        // Low center point (more conservative)
        let conservative = ScoreNormalizer::new(300.0, 0.01);

        // High center point (more lenient)
        let lenient = ScoreNormalizer::new(700.0, 0.01);

        let raw = 500;
        let conservative_score = conservative.normalize(raw);
        let lenient_score = lenient.normalize(raw);

        // Conservative should give higher score for same raw input
        assert!(conservative_score > lenient_score);
    }
}
