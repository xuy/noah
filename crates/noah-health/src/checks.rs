use serde::{Deserialize, Serialize};

/// Status of a single health check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

impl CheckStatus {
    pub fn points(self) -> u8 {
        match self {
            CheckStatus::Pass => 100,
            CheckStatus::Warn => 50,
            CheckStatus::Fail => 0,
        }
    }
}

/// Health check categories with their weights (must sum to 100).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Security,
    Updates,
    Performance,
    Backups,
    Network,
}

impl Category {
    pub fn weight(self) -> u8 {
        match self {
            Category::Security => 30,
            Category::Updates => 25,
            Category::Performance => 20,
            Category::Backups => 15,
            Category::Network => 10,
        }
    }

    pub fn all() -> &'static [Category] {
        &[
            Category::Security,
            Category::Updates,
            Category::Performance,
            Category::Backups,
            Category::Network,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Security => "Security",
            Category::Updates => "Updates",
            Category::Performance => "Performance",
            Category::Backups => "Backups",
            Category::Network => "Network",
        }
    }
}
