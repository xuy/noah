mod checks;
mod score;

pub use checks::{Category, CheckStatus};
pub use score::{CategoryScore, CheckResult, HealthScore, compute_score};
