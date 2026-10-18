use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::models::journal::{JournalEntry, Mood};

/// Persistencia de `journal_entries`: una entrada por perfil y día.
///
/// La clave pública es `(profile_id, date)` —un diario se lee y se escribe por
/// día—, así que `upsert`, `get` y `delete` se direccionan por el par; el `id`
/// (UUID) se conserva para referencias internas, pero no es la puerta de
/// entrada.
pub struct JournalRepo;

impl JournalRepo {
    /// Crea la entrada del día, o **reemplaza** la que ya existe.
    ///
    /// Siempre queda una sola fila por `(profile_id, date)`: el conflicto sobre
    /// el `UNIQUE` de la tabla actualiza la fila en su sitio, de modo que su
    /// `id` y su `created_at` originales sobreviven mientras `updated_at` pasa a
    /// `datetime('now')`.
    ///
    /// `created_at` y `updated_at` los pone la base de datos —los valores que
    /// lleve `entry` se ignoran— y la fila se relee al final, así que el
    /// [`JournalEntry`] devuelto es lo que quedó almacenado, no lo que se pidió.
    pub async fn upsert(
        pool: &SqlitePool,
        entry: &JournalEntry,
    ) -> Result<JournalEntry, sqlx::Error> {
        sqlx::query(
            "INSERT INTO journal_entries \
             (id, profile_id, date, mood, content, achievements, struggles, tags) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
             ON CONFLICT(profile_id, date) DO UPDATE SET \
                mood = excluded.mood, \
                content = excluded.content, \
                achievements = excluded.achievements, \
                struggles = excluded.struggles, \
                tags = excluded.tags, \
                updated_at = datetime('now')",
        )
        .bind(&entry.id)
        .bind(&entry.profile_id)
        .bind(&entry.date)
        .bind(entry.mood.map(|mood| mood.as_str()))
        .bind(&entry.content)
        .bind(encode_list(&entry.achievements))
        .bind(encode_list(&entry.struggles))
        .bind(encode_list(&entry.tags))
        .execute(pool)
        .await?;

        Self::get(pool, &entry.profile_id, &entry.date)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// La entrada de ese perfil y día, o `None` si ese día no está escrito.
    pub async fn get(
        pool: &SqlitePool,
        profile_id: &str,
        date: &str,
    ) -> Result<Option<JournalEntry>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, profile_id, date, mood, content, achievements, struggles, tags, \
             created_at, updated_at \
             FROM journal_entries WHERE profile_id = ?1 AND date = ?2",
        )
        .bind(profile_id)
        .bind(date)
        .fetch_optional(pool)
        .await?;

        row.as_ref().map(entry_from_row).transpose()
    }

    /// Las entradas del rango **inclusivo** `[from, to]`, de la más reciente a la
    /// más antigua y como mucho `limit`.
    ///
    /// La comparación es lexicográfica sobre la fecha ISO, que para
    /// `YYYY-MM-DD` es también la cronológica.
    pub async fn list(
        pool: &SqlitePool,
        profile_id: &str,
        from: &str,
        to: &str,
        limit: i64,
    ) -> Result<Vec<JournalEntry>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, profile_id, date, mood, content, achievements, struggles, tags, \
             created_at, updated_at \
             FROM journal_entries \
             WHERE profile_id = ?1 AND date >= ?2 AND date <= ?3 \
             ORDER BY date DESC LIMIT ?4",
        )
        .bind(profile_id)
        .bind(from)
        .bind(to)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        rows.iter().map(entry_from_row).collect()
    }

    /// `true` si borró la entrada de ese día, `false` si no había ninguna.
    pub async fn delete(
        pool: &SqlitePool,
        profile_id: &str,
        date: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM journal_entries WHERE profile_id = ?1 AND date = ?2")
            .bind(profile_id)
            .bind(date)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Serializa la lista para su columna `TEXT`: una lista vacía se guarda como
/// `[]`, nunca como `NULL`, para que no haya ambigüedad al leerla. `serde_json`
/// pasa por `Display`, así que esto no puede fallar.
fn encode_list(items: &[String]) -> String {
    let value = serde_json::Value::Array(
        items
            .iter()
            .map(|item| serde_json::Value::String(item.clone()))
            .collect(),
    );
    value.to_string()
}

/// Mapea una fila de `journal_entries` al modelo, con `NULL` en los tres campos
/// con estructura leído como lista vacía y `NULL` en `mood` como `None`.
fn entry_from_row(row: &SqliteRow) -> Result<JournalEntry, sqlx::Error> {
    Ok(JournalEntry {
        id: row.try_get("id")?,
        profile_id: row.try_get("profile_id")?,
        date: row.try_get("date")?,
        mood: decode_mood(row.try_get("mood")?)?,
        content: row.try_get("content")?,
        achievements: decode_list(row.try_get("achievements")?)?,
        struggles: decode_list(row.try_get("struggles")?)?,
        tags: decode_list(row.try_get("tags")?)?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

/// `NULL` es «sin ánimo», no un error; cualquier otra cosa pasa por la
/// deserialización estricta de [`Mood`], de modo que un valor fuera del conjunto
/// guardado por la puerta de atrás (la tabla no lleva `CHECK`) salga como error
/// en vez de tragarse el dato.
fn decode_mood(raw: Option<String>) -> Result<Option<Mood>, sqlx::Error> {
    match raw {
        None => Ok(None),
        Some(raw) => serde_json::from_value(serde_json::Value::String(raw))
            .map(Some)
            .map_err(|error| sqlx::Error::Decode(Box::new(error))),
    }
}

/// `NULL` es la lista vacía; un JSON mal formado es un error, nunca una lista
/// vacía silenciosa.
fn decode_list(raw: Option<String>) -> Result<Vec<String>, sqlx::Error> {
    match raw {
        None => Ok(Vec::new()),
        Some(raw) => {
            serde_json::from_str(&raw).map_err(|error| sqlx::Error::Decode(Box::new(error)))
        }
    }
}
