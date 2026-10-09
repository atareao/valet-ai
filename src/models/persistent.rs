use serde::{Deserialize, Serialize};

/// Layer C (persistent memory): the single logical row of stable user state —
/// the user profile and the system rules.
///
/// Maps to the `persistent_memory` table created by migration
/// `20261002000001_persistent_memory.sql`. The only row has
/// `id = "global_state"` (see [`crate::persistent_memory::GLOBAL_STATE_ID`]).
///
/// The absence of a row is not an error: it means "empty state". `payload` is
/// the raw JSON text of the versioned object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistentMemory {
    pub id: String,
    pub payload: String,
    /// RFC 3339 timestamp written by Rust (never by the LLM).
    pub updated_at: String,
}
