use serde::{Deserialize, Serialize};

/// El conjunto cerrado de ánimos que puede llevar una entrada de la bitácora.
///
/// La tabla `journal_entries` NO lleva un `CHECK` sobre `mood` (ver la
/// migración): el conjunto vive aquí, de modo que ampliarlo no obliga a
/// reconstruir la tabla. La serialización es `snake_case` y **estricta**: un
/// valor fuera del conjunto es un error de deserialización, nunca un ánimo por
/// defecto que se cuele en el diario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood {
    Great,
    Good,
    Neutral,
    Bad,
    Terrible,
}

impl Mood {
    /// La cadena que se persiste en la columna `mood`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Mood::Great => "great",
            Mood::Good => "good",
            Mood::Neutral => "neutral",
            Mood::Bad => "bad",
            Mood::Terrible => "terrible",
        }
    }
}

/// Un día de la bitácora del usuario.
///
/// El `id` (UUID) es interno: la clave pública es `(profile_id, date)` —un
/// diario se lee y se escribe por día—. El ánimo es opcional porque hay días que
/// se cuentan sin etiquetarlo, y `achievements`, `struggles` y `tags` se exponen
/// siempre como listas: un `NULL` en la base de datos se lee como lista vacía.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub profile_id: String,
    /// Fecha ISO `YYYY-MM-DD`, en la zona horaria del usuario.
    pub date: String,
    pub mood: Option<Mood>,
    /// El relato del día.
    pub content: String,
    pub achievements: Vec<String>,
    pub struggles: Vec<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}
