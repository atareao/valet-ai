use chrono::Utc;
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::models::message::estimate_markdown_tokens_heuristic;
use crate::models::Message;

pub struct MessagesRepo;

impl MessagesRepo {
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        pool: &SqlitePool,
        role: &str,
        content: &str,
        tool_calls: Option<&Value>,
        tool_results: Option<&Value>,
        location: Option<&str>,
        tools_used: Option<&str>,
        collapse_threshold: usize,
        on_collapse_needed: Option<Box<dyn Fn(String) + Send>>,
    ) -> Result<Message, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let tc = tool_calls.map(|v| v.to_string());
        let tr = tool_results.map(|v| v.to_string());
        let tokens = estimate_markdown_tokens_heuristic(content);

        sqlx::query(
            "INSERT INTO messages (id, role, content, tool_calls, tool_results, location, tools_used, tokens_count, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(&id)
        .bind(role)
        .bind(content)
        .bind(&tc)
        .bind(&tr)
        .bind(location)
        .bind(tools_used)
        .bind(tokens as i64)
        .bind(&now)
        .execute(pool)
        .await?;

        // If tokens exceed threshold, invoke collapse callback
        if tokens >= collapse_threshold {
            if let Some(ref callback) = on_collapse_needed {
                callback(id.clone());
            }
        }

        Ok(Message {
            id,
            role: role.to_string(),
            content: content.to_string(),
            tool_calls: tool_calls.cloned(),
            tool_results: tool_results.cloned(),
            tokens_count: tokens,
            collapsed_content: None,
            collapsed_tokens_count: 0,
            is_indexed: false,
            summary_ref: None,
            location: location.map(|s| s.to_string()),
            tools_used: tools_used.map(|s| s.to_string()),
            widgets: None,
            created_at: now,
        })
    }

    pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Message>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, role, content, tool_calls, tool_results, \
             tokens_count, collapsed_content, collapsed_tokens_count, is_indexed, summary_ref, location, tools_used, created_at, widgets \
             FROM messages WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => {
                let tc: Option<String> = r.get(3);
                let tr: Option<String> = r.get(4);
                Ok(Some(Message {
                    id: r.get(0),
                    role: r.get(1),
                    content: r.get(2),
                    tool_calls: tc.and_then(|s| serde_json::from_str(&s).ok()),
                    tool_results: tr.and_then(|s| serde_json::from_str(&s).ok()),
                    tokens_count: r.get::<i64, _>(5) as usize,
                    collapsed_content: r.get(6),
                    collapsed_tokens_count: r.get::<i64, _>(7) as usize,
                    is_indexed: r.get(8),
                    summary_ref: r.get(9),
                    location: r.get(10),
                    tools_used: r.get(11),
                    widgets: r
                        .get::<Option<String>, _>(13)
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    created_at: r.get(12),
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn list_all(
        pool: &SqlitePool,
        limit: i64,
        cursor: Option<&str>,
    ) -> Result<(Vec<Message>, Option<String>), sqlx::Error> {
        let actual_limit = limit.clamp(1, 100);

        let rows = match cursor {
            Some(c) => {
                sqlx::query(
                    "SELECT id, role, content, tool_calls, tool_results, \
                     tokens_count, collapsed_content, collapsed_tokens_count, is_indexed, summary_ref, location, tools_used, created_at, widgets \
FROM messages WHERE created_at < ?1 \
                      ORDER BY created_at DESC LIMIT ?2",
                )
                .bind(c)
                .bind(actual_limit + 1)
                .fetch_all(pool)
                .await?
            }
            None => {
                sqlx::query(
                    "SELECT id, role, content, tool_calls, tool_results, \
                     tokens_count, collapsed_content, collapsed_tokens_count, is_indexed, summary_ref, location, tools_used, created_at, widgets \
                     FROM messages \
                     ORDER BY created_at DESC LIMIT ?1",
                )
                .bind(actual_limit + 1)
                .fetch_all(pool)
                .await?
            }
        };

        let mut items: Vec<Message> = Vec::with_capacity(rows.len());
        for r in &rows {
            let tc: Option<String> = r.get(3);
            let tr: Option<String> = r.get(4);
            items.push(Message {
                id: r.get(0),
                role: r.get(1),
                content: r.get(2),
                tool_calls: tc.and_then(|s| serde_json::from_str(&s).ok()),
                tool_results: tr.and_then(|s| serde_json::from_str(&s).ok()),
                tokens_count: r.get::<i64, _>(5) as usize,
                collapsed_content: r.get(6),
                collapsed_tokens_count: r.get::<i64, _>(7) as usize,
                is_indexed: r.get(8),
                summary_ref: r.get(9),
                location: r.get(10),
                tools_used: r.get(11),
                widgets: r
                    .get::<Option<String>, _>(13)
                    .and_then(|s| serde_json::from_str(&s).ok()),
                created_at: r.get(12),
            });
        }

        let has_more = items.len() > actual_limit as usize;
        let mut data: Vec<Message> = if has_more {
            items[..actual_limit as usize].to_vec()
        } else {
            items
        };
        data.reverse();
        let next_cursor = if has_more {
            data.first().map(|m| m.created_at.clone())
        } else {
            None
        };
        Ok((data, next_cursor))
    }

    pub async fn list_by_token_budget(
        pool: &SqlitePool,
        max_tokens: usize,
    ) -> Result<Vec<Message>, sqlx::Error> {
        let max_tokens_i64 = max_tokens as i64;
        let rows = sqlx::query(
            "WITH RankedMessages AS (
                SELECT id, role,
                       CASE WHEN collapsed_content IS NOT NULL THEN collapsed_content ELSE content END AS effective_content,
                       CASE WHEN collapsed_content IS NOT NULL THEN collapsed_tokens_count ELSE tokens_count END AS effective_tokens,
                       tool_calls, tool_results,
                       collapsed_content, collapsed_tokens_count,
                       is_indexed, summary_ref, location, tools_used, created_at, widgets,
                       SUM(CASE WHEN collapsed_content IS NOT NULL THEN collapsed_tokens_count ELSE tokens_count END)
                           OVER (ORDER BY created_at DESC ROWS UNBOUNDED PRECEDING) AS cumulative_tokens
                FROM messages
            )
            SELECT id, role, effective_content,
                   tool_calls, tool_results,
                   effective_tokens, collapsed_content, collapsed_tokens_count,
                   is_indexed, summary_ref, location, tools_used, created_at, widgets
            FROM RankedMessages
            WHERE cumulative_tokens <= ?1
            ORDER BY created_at ASC",
        )
        .bind(max_tokens_i64)
        .fetch_all(pool)
        .await?;

        let mut messages = Vec::with_capacity(rows.len());
        for r in &rows {
            let tc: Option<String> = r.get(3);
            let tr: Option<String> = r.get(4);
            messages.push(Message {
                id: r.get(0),
                role: r.get(1),
                content: r.get(2),
                tool_calls: tc.and_then(|s| serde_json::from_str(&s).ok()),
                tool_results: tr.and_then(|s| serde_json::from_str(&s).ok()),
                tokens_count: r.get::<i64, _>(5) as usize,
                collapsed_content: r.get(6),
                collapsed_tokens_count: r.get::<i64, _>(7) as usize,
                is_indexed: r.get(8),
                summary_ref: r.get(9),
                location: r.get(10),
                tools_used: r.get(11),
                widgets: r
                    .get::<Option<String>, _>(13)
                    .and_then(|s| serde_json::from_str(&s).ok()),
                created_at: r.get(12),
            });
        }
        Ok(messages)
    }

    pub async fn delete_all(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM messages").execute(pool).await?;
        Ok(result.rows_affected())
    }

    /// Persist the rendered widgets of a message as a JSON array in the
    /// `widgets` column, overwriting any previous value.
    pub async fn set_widgets(
        pool: &SqlitePool,
        id: &str,
        widgets: &Value,
    ) -> Result<(), sqlx::Error> {
        let serialized = widgets.to_string();
        sqlx::query("UPDATE messages SET widgets = ?1 WHERE id = ?2")
            .bind(&serialized)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    /// Create an in-memory SQLite pool with the messages table needed for tests.
    async fn setup_pool() -> Result<SqlitePool, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_calls TEXT,
                tool_results TEXT,
                location TEXT,
                tokens_count INTEGER NOT NULL DEFAULT 0,
                collapsed_content TEXT,
                collapsed_tokens_count INTEGER NOT NULL DEFAULT 0,
                is_indexed INTEGER NOT NULL DEFAULT 0,
                summary_ref TEXT,
                tools_used TEXT,
                widgets TEXT,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await?;

        Ok(pool)
    }

    #[tokio::test]
    async fn test_create_message() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let msg = MessagesRepo::create(&pool, "user", "Hello", None, None, None, None, 2000, None)
            .await?;
        assert!(!msg.id.is_empty());
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello");

        Ok(())
    }

    #[tokio::test]
    async fn test_create_message_with_tool_calls() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let tool_calls = json!([{"name": "get_weather", "args": {"city": "Madrid"}}]);
        let msg = MessagesRepo::create(
            &pool,
            "assistant",
            "Let me check",
            Some(&tool_calls),
            None,
            None,
            None,
            2000,
            None,
        )
        .await?;
        assert!(msg.tool_calls.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let found = MessagesRepo::find_by_id(&pool, "nonexistent").await?;
        assert!(found.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_list_all() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        MessagesRepo::create(&pool, "user", "First", None, None, None, None, 2000, None).await?;
        MessagesRepo::create(
            &pool,
            "assistant",
            "Second",
            None,
            None,
            None,
            None,
            2000,
            None,
        )
        .await?;
        let (msgs, cursor) = MessagesRepo::list_all(&pool, 10, None).await?;
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].content, "First");
        assert_eq!(msgs[1].content, "Second");
        assert!(cursor.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_list_all_with_pagination() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        MessagesRepo::create(&pool, "user", "Msg 1", None, None, None, None, 2000, None).await?;
        MessagesRepo::create(&pool, "user", "Msg 2", None, None, None, None, 2000, None).await?;
        MessagesRepo::create(&pool, "user", "Msg 3", None, None, None, None, 2000, None).await?;

        let (page1, cursor) = MessagesRepo::list_all(&pool, 2, None).await?;
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].content, "Msg 2");
        assert_eq!(page1[1].content, "Msg 3");
        assert!(cursor.is_some());

        let (page2, cursor2) = MessagesRepo::list_all(&pool, 2, cursor.as_deref()).await?;
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].content, "Msg 1");
        assert!(cursor2.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_list_all_returns_most_recent() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        for i in 1..=55 {
            let content = format!("Msg {}", i);
            MessagesRepo::create(&pool, "user", &content, None, None, None, None, 2000, None)
                .await?;
        }

        let (msgs, cursor) = MessagesRepo::list_all(&pool, 50, None).await?;
        assert_eq!(msgs.len(), 50);
        assert_eq!(msgs[0].content, "Msg 6");
        assert_eq!(msgs[49].content, "Msg 55");
        assert!(cursor.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_list_all_pagination_backwards() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        for i in 1..=55 {
            let content = format!("Msg {}", i);
            MessagesRepo::create(&pool, "user", &content, None, None, None, None, 2000, None)
                .await?;
        }

        let (page1, cursor) = MessagesRepo::list_all(&pool, 50, None).await?;
        assert_eq!(page1.len(), 50);
        assert_eq!(page1[0].content, "Msg 6");
        assert_eq!(page1[49].content, "Msg 55");
        assert!(cursor.is_some());

        let (page2, cursor2) = MessagesRepo::list_all(&pool, 50, cursor.as_deref()).await?;
        assert_eq!(page2.len(), 5);
        assert_eq!(page2[0].content, "Msg 1");
        assert_eq!(page2[4].content, "Msg 5");
        assert!(cursor2.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_create_message_with_location() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let msg = MessagesRepo::create(
            &pool,
            "user",
            "Hello from Madrid",
            None,
            None,
            Some("Madrid"),
            None,
            2000,
            None,
        )
        .await?;
        assert_eq!(msg.location, Some("Madrid".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_create_message_without_location() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let msg = MessagesRepo::create(&pool, "user", "Hello", None, None, None, None, 2000, None)
            .await?;
        assert_eq!(msg.location, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_find_by_id_returns_location() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let created = MessagesRepo::create(
            &pool,
            "user",
            "Hello from Barcelona",
            None,
            None,
            Some("Barcelona"),
            None,
            2000,
            None,
        )
        .await?;
        let found = MessagesRepo::find_by_id(&pool, &created.id).await?;
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.location, Some("Barcelona".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_all() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        MessagesRepo::create(&pool, "user", "Msg", None, None, None, None, 2000, None).await?;
        let deleted = MessagesRepo::delete_all(&pool).await?;
        assert_eq!(deleted, 1);

        Ok(())
    }

    /// After creating a message, `tokens_count` must be automatically
    /// computed from the content.
    #[tokio::test]
    async fn test_create_message_computes_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let content = "Hello, how are you? This is a test message.";
        let msg = MessagesRepo::create(&pool, "user", content, None, None, None, None, 2000, None)
            .await?;
        assert!(
            msg.tokens_count > 0,
            "tokens_count should be > 0, got {}",
            msg.tokens_count
        );
        let expected = crate::models::message::estimate_markdown_tokens_heuristic(content);
        assert_eq!(
            msg.tokens_count, expected,
            "tokens_count should equal estimate_markdown_tokens_heuristic(content)"
        );

        Ok(())
    }

    /// A short message (below collapse threshold) must NOT have
    /// a collapsed_content.
    #[tokio::test]
    async fn test_create_message_short_no_collapse() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let content = "Short message.";
        let msg = MessagesRepo::create(&pool, "user", content, None, None, None, None, 2000, None)
            .await?;
        assert!(
            msg.collapsed_content.is_none(),
            "Short messages should not have collapsed_content"
        );
        let estimated = crate::models::message::estimate_markdown_tokens_heuristic(content);
        assert!(
            estimated < 2000,
            "Short message should be below collapse threshold"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_create_message_long_triggers_collapse() -> Result<(), Box<dyn std::error::Error>>
    {
        let pool = setup_pool().await?;
        // 2000 words → ~2660 tokens (well above 2000 threshold)
        let long_content = "x ".repeat(2000);

        let collapse_requested = std::sync::Arc::new(std::sync::Mutex::new(false));
        let captured_id = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let cr_clone = collapse_requested.clone();
        let ci_clone = captured_id.clone();

        let callback: Option<Box<dyn Fn(String) + Send>> = Some(Box::new(move |msg_id: String| {
            *cr_clone.lock().unwrap() = true;
            *ci_clone.lock().unwrap() = msg_id;
        }));

        let msg = MessagesRepo::create(
            &pool,
            "user",
            &long_content,
            None,
            None,
            None,
            None,
            2000,
            callback,
        )
        .await?;

        // The estimated tokens must exceed the threshold
        let estimated = crate::models::message::estimate_markdown_tokens_heuristic(&long_content);
        assert!(
            estimated > 2000,
            "Long content should exceed collapse threshold (estimated={})",
            estimated
        );
        assert!(
            msg.tokens_count >= estimated,
            "tokens_count should be >= estimated tokens"
        );
        // When the threshold is exceeded, the collapse callback must fire
        assert!(
            *collapse_requested.lock().unwrap(),
            "Collapse callback should have been invoked for long message"
        );
        assert_eq!(
            *captured_id.lock().unwrap(),
            msg.id,
            "Callback should receive the message id"
        );

        Ok(())
    }

    // ── list_by_token_budget tests ──────────────────────────────────────────

    #[tokio::test]
    async fn test_list_by_token_budget_selects_within_budget(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;

        // ~1000 tokens: 752 words * 1.33 = 1000
        let content_1k = "a ".repeat(752);
        // ~2000 tokens: 1504 words * 1.33 = 2000
        let content_2k = "b ".repeat(1504);

        let msg1 = MessagesRepo::create(
            &pool,
            "user",
            &content_1k,
            None,
            None,
            None,
            None,
            99999,
            None,
        )
        .await?;
        let msg2 = MessagesRepo::create(
            &pool,
            "user",
            &content_2k,
            None,
            None,
            None,
            None,
            99999,
            None,
        )
        .await?;
        let msg3 = MessagesRepo::create(
            &pool,
            "user",
            &content_1k,
            None,
            None,
            None,
            None,
            99999,
            None,
        )
        .await?;

        let result = MessagesRepo::list_by_token_budget(&pool, 3500).await?;

        // Should return the 2 most recent messages: msg2 (2000) + msg3 (1000) = 3000 ≤ 3500
        assert_eq!(result.len(), 2, "Should keep 2 messages within budget");
        assert_eq!(result[0].id, msg2.id, "Oldest kept message should be msg2");
        assert_eq!(result[1].id, msg3.id, "Newest kept message should be msg3");
        // msg1 (1000 tokens) excluded because adding it would exceed 3500
        let ids: Vec<&str> = result.iter().map(|m| m.id.as_str()).collect();
        assert!(
            !ids.contains(&msg1.id.as_str()),
            "Oldest message (msg1) should be excluded"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_list_by_token_budget_all_fit() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;

        // ~500 tokens: ceil(1736/3.5) + 4 = 500
        let content_500 = "a".repeat(1736);

        let msg1 = MessagesRepo::create(
            &pool,
            "user",
            &content_500,
            None,
            None,
            None,
            None,
            99999,
            None,
        )
        .await?;
        let msg2 = MessagesRepo::create(
            &pool,
            "user",
            &content_500,
            None,
            None,
            None,
            None,
            99999,
            None,
        )
        .await?;

        let result = MessagesRepo::list_by_token_budget(&pool, 2000).await?;

        assert_eq!(result.len(), 2, "Both messages should fit within budget");
        assert_eq!(result[0].id, msg1.id);
        assert_eq!(result[1].id, msg2.id);

        Ok(())
    }

    #[tokio::test]
    async fn test_list_by_token_budget_uses_collapsed_tokens(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;

        // Create a message with ~3000 tokens
        let content_3k = "a".repeat(10486); // ceil(10486/3.5) + 4 = 3000
        let msg = MessagesRepo::create(
            &pool,
            "user",
            &content_3k,
            None,
            None,
            None,
            None,
            99999,
            None,
        )
        .await?;

        // Simulate collapsing: set collapsed fields via raw SQL
        let collapsed_text = "Short collapsed summary";
        sqlx::query(
            "UPDATE messages SET collapsed_content = ?1, collapsed_tokens_count = ?2 WHERE id = ?3",
        )
        .bind(collapsed_text)
        .bind(200_i64)
        .bind(&msg.id)
        .execute(&pool)
        .await?;

        // max_tokens=500, collapsed_tokens_count=200 ≤ 500 → included
        let result = MessagesRepo::list_by_token_budget(&pool, 500).await?;

        assert_eq!(result.len(), 1, "Collapsed message should be included");
        assert_eq!(
            result[0].content, collapsed_text,
            "Returned content should be the collapsed content"
        );
        assert_eq!(
            result[0].collapsed_tokens_count, 200,
            "Collapsed tokens count should be preserved"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_list_by_token_budget_zero_budget() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;

        let content = "Some message content";
        MessagesRepo::create(&pool, "user", content, None, None, None, None, 99999, None).await?;
        MessagesRepo::create(
            &pool,
            "assistant",
            "response",
            None,
            None,
            None,
            None,
            99999,
            None,
        )
        .await?;

        let result = MessagesRepo::list_by_token_budget(&pool, 0).await?;

        assert!(result.is_empty(), "Zero budget should return empty list");

        Ok(())
    }

    #[tokio::test]
    async fn test_list_by_token_budget_empty() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;

        let result = MessagesRepo::list_by_token_budget(&pool, 10000).await?;

        assert!(result.is_empty(), "Empty store should return empty list");

        Ok(())
    }

    // ── widgets persistence ─────────────────────────────────────────────────

    /// `set_widgets` persists the JSON on the row and `find_by_id` returns it
    /// deserialized.
    #[tokio::test]
    async fn test_set_widgets_persists_and_find_by_id_returns_them(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let msg = MessagesRepo::create(
            &pool,
            "assistant",
            "Aquí tienes",
            None,
            None,
            None,
            None,
            2000,
            None,
        )
        .await?;

        let widgets = json!([
            {"id": "w1", "name": "LocationWidget", "data": {"latitude": 1.0, "longitude": 2.0}}
        ]);
        MessagesRepo::set_widgets(&pool, &msg.id, &widgets).await?;

        let found = MessagesRepo::find_by_id(&pool, &msg.id)
            .await?
            .expect("the message must exist");
        assert_eq!(
            found.widgets.as_ref(),
            Some(&widgets),
            "find_by_id must return the persisted widgets"
        );

        Ok(())
    }

    /// A message created without widgets has `widgets = None`.
    #[tokio::test]
    async fn test_widgets_is_none_when_not_set() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let msg = MessagesRepo::create(
            &pool,
            "assistant",
            "Sin widgets",
            None,
            None,
            None,
            None,
            2000,
            None,
        )
        .await?;

        let found = MessagesRepo::find_by_id(&pool, &msg.id)
            .await?
            .expect("the message must exist");
        assert!(
            found.widgets.is_none(),
            "widgets must be None when never set"
        );

        Ok(())
    }

    /// `list_all` includes the persisted widgets in the returned message.
    #[tokio::test]
    async fn test_list_all_returns_widgets() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let msg = MessagesRepo::create(
            &pool,
            "assistant",
            "Checklist",
            None,
            None,
            None,
            None,
            2000,
            None,
        )
        .await?;
        let widgets = json!([{"id": "w1", "name": "Checklist", "data": {}}]);
        MessagesRepo::set_widgets(&pool, &msg.id, &widgets).await?;

        let (msgs, _) = MessagesRepo::list_all(&pool, 10, None).await?;
        let listed = msgs
            .iter()
            .find(|m| m.id == msg.id)
            .expect("the message must be listed");
        assert_eq!(
            listed.widgets.as_ref(),
            Some(&widgets),
            "list_all must include the persisted widgets"
        );

        Ok(())
    }

    /// A second `set_widgets` overwrites the previous list (no duplication).
    #[tokio::test]
    async fn test_set_widgets_overwrites() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let msg = MessagesRepo::create(
            &pool,
            "assistant",
            "Dos widgets",
            None,
            None,
            None,
            None,
            2000,
            None,
        )
        .await?;

        let first = json!([{"id": "w1", "name": "A", "data": {}}]);
        let second = json!([
            {"id": "w2", "name": "B", "data": {}},
            {"id": "w3", "name": "C", "data": {}}
        ]);
        MessagesRepo::set_widgets(&pool, &msg.id, &first).await?;
        MessagesRepo::set_widgets(&pool, &msg.id, &second).await?;

        let found = MessagesRepo::find_by_id(&pool, &msg.id)
            .await?
            .expect("the message must exist");
        assert_eq!(
            found.widgets.as_ref(),
            Some(&second),
            "the last call must overwrite the previous widgets"
        );

        Ok(())
    }

    /// `list_by_token_budget` also returns the persisted widgets.
    #[tokio::test]
    async fn test_list_by_token_budget_returns_widgets() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_pool().await?;
        let msg = MessagesRepo::create(
            &pool,
            "assistant",
            "Widget",
            None,
            None,
            None,
            None,
            99999,
            None,
        )
        .await?;
        let widgets = json!([{"id": "w1", "name": "Checklist", "data": {}}]);
        MessagesRepo::set_widgets(&pool, &msg.id, &widgets).await?;

        let result = MessagesRepo::list_by_token_budget(&pool, 10000).await?;
        let listed = result
            .iter()
            .find(|m| m.id == msg.id)
            .expect("the message must be within budget");
        assert_eq!(
            listed.widgets.as_ref(),
            Some(&widgets),
            "list_by_token_budget must include the persisted widgets"
        );

        Ok(())
    }
}
