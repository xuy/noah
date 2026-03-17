use serde::{Deserialize, Serialize};

use crate::checks::{Category, CheckStatus};

/// Result of a single health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// Unique check identifier, e.g. "security.firewall".
    pub id: String,
    /// Which category this check belongs to.
    pub category: Category,
    /// Human-readable label, e.g. "Firewall".
    pub label: String,
    /// Pass / Warn / Fail.
    pub status: CheckStatus,
    /// Optional detail text, e.g. "Firewall is enabled".
    pub detail: String,
}

/// Aggregated score for a single category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryScore {
    pub category: Category,
    /// 0-100 score for this category.
    pub score: u8,
    /// Letter grade: A, B, C, D, or F.
    pub grade: char,
    /// Individual check results in this category.
    pub checks: Vec<CheckResult>,
}

/// Overall device health score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScore {
    /// Weighted overall score 0-100.
    pub overall_score: u8,
    /// Overall letter grade.
    pub overall_grade: char,
    /// Per-category breakdown.
    pub categories: Vec<CategoryScore>,
    /// When this score was computed (RFC 3339).
    pub computed_at: String,
    /// Device identifier (optional).
    pub device_id: Option<String>,
}

fn grade_for(score: u8) -> char {
    match score {
        90..=100 => 'A',
        75..=89 => 'B',
        60..=74 => 'C',
        45..=59 => 'D',
        _ => 'F',
    }
}

/// Compute a health score from a flat list of check results.
///
/// Categories with no checks are omitted from the result and do not
/// contribute to the overall weighted score.
pub fn compute_score(
    checks: Vec<CheckResult>,
    device_id: Option<String>,
    enabled_categories: Option<&[Category]>,
) -> HealthScore {
    let checks = if let Some(enabled) = enabled_categories {
        checks.into_iter().filter(|c| enabled.contains(&c.category)).collect()
    } else {
        checks
    };

    let mut category_scores: Vec<CategoryScore> = Vec::new();

    for &cat in Category::all() {
        let cat_checks: Vec<&CheckResult> = checks.iter().filter(|c| c.category == cat).collect();
        if cat_checks.is_empty() {
            continue;
        }

        let total: u32 = cat_checks.iter().map(|c| c.status.points() as u32).sum();
        let avg = (total / cat_checks.len() as u32).min(100) as u8;

        category_scores.push(CategoryScore {
            category: cat,
            score: avg,
            grade: grade_for(avg),
            checks: checks.iter().filter(|c| c.category == cat).cloned().collect(),
        });
    }

    // Weighted overall: only count categories that have checks.
    let total_weight: u32 = category_scores.iter().map(|cs| cs.category.weight() as u32).sum();
    let overall = if total_weight > 0 {
        let weighted_sum: u32 = category_scores
            .iter()
            .map(|cs| cs.score as u32 * cs.category.weight() as u32)
            .sum();
        (weighted_sum / total_weight).min(100) as u8
    } else {
        0
    };

    HealthScore {
        overall_score: overall,
        overall_grade: grade_for(overall),
        categories: category_scores,
        computed_at: chrono::Utc::now().to_rfc3339(),
        device_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(id: &str, cat: Category, status: CheckStatus) -> CheckResult {
        CheckResult {
            id: id.to_string(),
            category: cat,
            label: id.to_string(),
            status,
            detail: String::new(),
        }
    }

    #[test]
    fn all_pass_gives_a() {
        let checks = vec![
            check("sec.fw", Category::Security, CheckStatus::Pass),
            check("sec.fv", Category::Security, CheckStatus::Pass),
            check("upd.os", Category::Updates, CheckStatus::Pass),
        ];
        let score = compute_score(checks, None, None);
        assert_eq!(score.overall_grade, 'A');
        assert!(score.overall_score >= 90);
    }

    #[test]
    fn all_fail_gives_f() {
        let checks = vec![
            check("sec.fw", Category::Security, CheckStatus::Fail),
            check("upd.os", Category::Updates, CheckStatus::Fail),
        ];
        let score = compute_score(checks, None, None);
        assert_eq!(score.overall_grade, 'F');
        assert_eq!(score.overall_score, 0);
    }

    #[test]
    fn mixed_scores() {
        let checks = vec![
            check("sec.fw", Category::Security, CheckStatus::Pass),  // 100
            check("sec.fv", Category::Security, CheckStatus::Fail),  // 0
            // Security avg = 50, weight 30
            check("upd.os", Category::Updates, CheckStatus::Warn),   // 50
            // Updates avg = 50, weight 25
        ];
        let score = compute_score(checks, None, None);
        // weighted = (50*30 + 50*25) / (30+25) = 2750/55 = 50
        assert_eq!(score.overall_score, 50);
        assert_eq!(score.overall_grade, 'D');
    }

    #[test]
    fn empty_checks_gives_zero() {
        let score = compute_score(vec![], None, None);
        assert_eq!(score.overall_score, 0);
        assert_eq!(score.overall_grade, 'F');
        assert!(score.categories.is_empty());
    }

    #[test]
    fn grade_boundaries() {
        assert_eq!(grade_for(100), 'A');
        assert_eq!(grade_for(90), 'A');
        assert_eq!(grade_for(89), 'B');
        assert_eq!(grade_for(75), 'B');
        assert_eq!(grade_for(74), 'C');
        assert_eq!(grade_for(60), 'C');
        assert_eq!(grade_for(59), 'D');
        assert_eq!(grade_for(45), 'D');
        assert_eq!(grade_for(44), 'F');
        assert_eq!(grade_for(0), 'F');
    }
}
