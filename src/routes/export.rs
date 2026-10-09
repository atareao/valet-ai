use crate::errors::AppError;
use axum::{extract::State, Json};
use serde_json::{json, Value};
use sqlx::{Column, Row, SqlitePool};

use crate::AppState;

/// Tables that may be exported. `export_table` interpolates the name into SQL,
/// so only these hard-coded names are ever accepted.
const EXPORTABLE_TABLES: &[&str] = &[
    "profiles",
    "messages",
    "events",
    "tasks",
    "notes",
    "reminders",
    "memory",
    "tools",
];

/// Export all data from the database as a single JSON object.
///
/// Returns a JSON object with one key per table, each containing an array
/// of rows serialized as objects with column-name keys and string/number/null values.
pub async fn export_data(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    Ok(Json(export_all_tables(&state.db).await))
}

async fn export_all_tables(pool: &SqlitePool) -> Value {
    json!({
        "profiles": export_table(pool, "profiles").await,
        "messages": export_table(pool, "messages").await,
        "events": export_table(pool, "events").await,
        "tasks": export_table(pool, "tasks").await,
        "notes": export_table(pool, "notes").await,
        "reminders": export_table(pool, "reminders").await,
        "memory": export_table(pool, "memory").await,
        "tools": export_table(pool, "tools").await,
    })
}

async fn export_table(pool: &SqlitePool, table_name: &str) -> Value {
    if !EXPORTABLE_TABLES.contains(&table_name) {
        return json!([]);
    }

    let query_str = format!("SELECT * FROM \"{}\"", table_name);
    // SAFETY: `table_name` is validated against the hard-coded `EXPORTABLE_TABLES`
    // whitelist above, so no caller-controlled string is ever interpolated.
    let rows = match sqlx::query(sqlx::AssertSqlSafe(query_str))
        .fetch_all(pool)
        .await
    {
        Ok(r) => r,
        Err(_) => return json!([]),
    };

    if rows.is_empty() {
        return json!([]);
    }

    // Get column names from the first row's columns
    let columns: Vec<String> = rows[0]
        .columns()
        .iter()
        .map(|c| c.name().to_string())
        .collect();

    let results: Vec<Value> = rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for (i, name) in columns.iter().enumerate() {
                let value: Value = match row.try_get::<String, _>(i) {
                    Ok(v) => json!(v),
                    Err(_) => match row.try_get::<i64, _>(i) {
                        Ok(v) => json!(v),
                        Err(_) => Value::Null,
                    },
                };
                obj.insert(name.clone(), value);
            }
            Value::Object(obj)
        })
        .collect();
    json!(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn setup_pool() -> Result<SqlitePool, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();
        Ok(pool)
    }

    #[tokio::test]
    async fn test_export_table_empty() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let result = export_table(&pool, "profiles").await;
        assert_eq!(result, json!([]));
        Ok(())
    }

    #[tokio::test]
    async fn test_export_table_nonexistent() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let result = export_table(&pool, "nonexistent_table").await;
        assert_eq!(result, json!([]));
        Ok(())
    }

    #[tokio::test]
    async fn test_export_table_with_data() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        sqlx::query("INSERT INTO profiles (id, name, preferences) VALUES ('p1', 'Alice', '{}')")
            .execute(&pool)
            .await?;
        let result = export_table(&pool, "profiles").await;
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "Alice");
        Ok(())
    }
}
