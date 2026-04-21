//! Scoring system for scenario results.
//!
//! Satisfies Requirement: sandbox-manifest-scoring
//!
//! Scoring formula:
/// - Correctness × 0.35
/// - Latency × 0.20
/// - Scalability × 0.15
/// - Consistency × 0.15
/// - Robustness × 0.15
use serde::{Deserialize, Serialize};

/// Individual scoring breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringBreakdown {
    /// Correctness score (0.0 - 1.0).
    pub correctness: f64,
    /// Latency score (0.0 - 1.0).
    pub latency: f64,
    /// Scalability score (0.0 - 1.0).
    pub scalability: f64,
    /// Consistency score (0.0 - 1.0).
    pub consistency: f64,
    /// Robustness score (0.0 - 1.0).
    pub robustness: f64,
    /// Weighted total score (0.0 - 1.0).
    pub total: f64,
}

impl ScoringBreakdown {
    /// Calculates the weighted total score.
    pub fn calculate_total(&mut self) {
        self.total = self.correctness * 0.35
            + self.latency * 0.20
            + self.scalability * 0.15
            + self.consistency * 0.15
            + self.robustness * 0.15;
    }

    /// Returns the letter grade based on the total score.
    pub fn grade(&self) -> char {
        match self.total {
            0.90..=1.0 => 'A',
            0.80..=0.89 => 'B',
            0.70..=0.79 => 'C',
            0.60..=0.69 => 'D',
            _ => 'F',
        }
    }
}

impl Default for ScoringBreakdown {
    fn default() -> Self {
        Self {
            correctness: 0.0,
            latency: 0.0,
            scalability: 0.0,
            consistency: 0.0,
            robustness: 0.0,
            total: 0.0,
        }
    }
}

/// Scores a scenario result.
pub fn score_result(
    correctness: f64,
    latency: f64,
    scalability: f64,
    consistency: f64,
    robustness: f64,
) -> ScoringBreakdown {
    let mut breakdown = ScoringBreakdown {
        correctness,
        latency,
        scalability,
        consistency,
        robustness,
        total: 0.0,
    };
    breakdown.calculate_total();
    breakdown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scoring_calculation() {
        let breakdown = score_result(1.0, 1.0, 1.0, 1.0, 1.0);
        assert!((breakdown.total - 1.0).abs() < 0.001);
        assert_eq!(breakdown.grade(), 'A');
    }

    #[test]
    fn test_grade_boundaries() {
        let a = score_result(0.95, 0.95, 0.95, 0.95, 0.95);
        assert_eq!(a.grade(), 'A');

        let b = score_result(0.85, 0.85, 0.85, 0.85, 0.85);
        assert_eq!(b.grade(), 'B');

        let c = score_result(0.75, 0.75, 0.75, 0.75, 0.75);
        assert_eq!(c.grade(), 'C');

        let d = score_result(0.65, 0.65, 0.65, 0.65, 0.65);
        assert_eq!(d.grade(), 'D');

        let f = score_result(0.50, 0.50, 0.50, 0.50, 0.50);
        assert_eq!(f.grade(), 'F');
    }
}
