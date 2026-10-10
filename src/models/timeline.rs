use serde::{Deserialize, Serialize};

/// The closed set of categories a timeline event may carry.
///
/// Mirrors the `CHECK (category IN (...))` constraint of the
/// `timeline_events` table; keep both in sync.
pub const TIMELINE_CATEGORIES: [&str; 7] = [
    "sport",
    "lifestyle",
    "work",
    "shopping",
    "health",
    "social",
    "system",
];

/// Fallback category used when a category is missing or unknown.
pub const DEFAULT_TIMELINE_CATEGORY: &str = "lifestyle";

/// Whether `category` belongs to [`TIMELINE_CATEGORIES`].
pub fn is_valid_category(category: &str) -> bool {
    TIMELINE_CATEGORIES.contains(&category)
}

/// Return `category` when it is valid, otherwise [`DEFAULT_TIMELINE_CATEGORY`].
///
/// No trimming, no panic, no bookkeeping: an unknown category is folded into the
/// default rather than dropping the event it belongs to.
pub fn normalize_category(category: &str) -> String {
    if is_valid_category(category) {
        category.to_string()
    } else {
        DEFAULT_TIMELINE_CATEGORY.to_string()
    }
}

/// A persisted chronological fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct TimelineEvent {
    pub id: String,
    pub timestamp: String,
    pub category: String,
    pub fact: String,
    pub source_message_id: Option<String>,
    pub created_at: String,
}

/// An event about to be inserted; `created_at` is assigned by the database.
#[derive(Debug, Clone, PartialEq)]
pub struct NewTimelineEvent {
    pub id: String,
    pub timestamp: String,
    pub category: String,
    pub fact: String,
    pub source_message_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_categories_are_accepted() {
        for category in TIMELINE_CATEGORIES {
            assert!(
                is_valid_category(category),
                "'{category}' must be a valid category"
            );
        }
    }

    #[test]
    fn unknown_category_is_rejected() {
        assert!(!is_valid_category("random"));
    }

    #[test]
    fn empty_category_is_rejected() {
        assert!(!is_valid_category(""));
    }

    #[test]
    fn uppercase_category_is_rejected() {
        assert!(!is_valid_category("SPORT"));
    }

    #[test]
    fn normalize_keeps_valid_category() {
        assert_eq!(normalize_category("sport"), "sport");
        assert_eq!(normalize_category("system"), "system");
    }

    #[test]
    fn normalize_folds_unknown_to_default() {
        assert_eq!(normalize_category("random"), DEFAULT_TIMELINE_CATEGORY);
    }

    #[test]
    fn normalize_folds_empty_to_default() {
        assert_eq!(normalize_category(""), DEFAULT_TIMELINE_CATEGORY);
    }

    #[test]
    fn normalize_folds_uppercase_to_default() {
        assert_eq!(normalize_category("SPORT"), DEFAULT_TIMELINE_CATEGORY);
    }
}
