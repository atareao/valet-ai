use chrono::{
    DateTime, Datelike, Duration, FixedOffset, NaiveDate, NaiveDateTime, SecondsFormat, Timelike,
    Utc, Weekday,
};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub profile_id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub location: Option<String>,
    pub scope: String,
    pub category: String,
    pub all_day: bool,
    pub rrule: Option<String>,
    pub reminder_minutes_before: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

/// Traduce un límite de rango en texto a un instante.
/// - RFC 3339 con offset o sufijo `Z` → el instante tal cual.
/// - `YYYY-MM-DD` → día UTC completo: `T00:00:00Z`, o el inicio del día
///   siguiente cuando `upper = true`.
/// - Cualquier otro valor → `None` (límite no parseable).
pub fn parse_range_bound(raw: &str, upper: bool) -> Option<DateTime<FixedOffset>> {
    // RFC 3339 first: both "Z" and numeric offsets are accepted.
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Some(dt);
    }

    // Bare date, interpreted as a full UTC day.
    if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        let naive_dt = if upper {
            date.checked_add_days(chrono::Days::new(1))?
                .and_hms_opt(0, 0, 0)?
        } else {
            date.and_hms_opt(0, 0, 0)?
        };
        let utc = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
        return Some(utc.fixed_offset());
    }

    None
}

fn event_from_row(r: &sqlx::sqlite::SqliteRow) -> Event {
    Event {
        id: r.get(0),
        profile_id: r.get(1),
        title: r.get(2),
        description: r.get(3),
        start_time: r.get(4),
        end_time: r.get(5),
        location: r.get(6),
        scope: r.get(7),
        category: r.get(8),
        all_day: r.get(9),
        rrule: r.get(10),
        reminder_minutes_before: r.get(11),
        created_at: r.get(12),
        updated_at: r.get(13),
    }
}

pub struct EventsRepo;

impl EventsRepo {
    pub async fn create(pool: &SqlitePool, event: &Event) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO events (id, profile_id, title, description, start_time, end_time, location, scope, category, all_day, rrule, reminder_minutes_before, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )
        .bind(&event.id)
        .bind(&event.profile_id)
        .bind(&event.title)
        .bind(&event.description)
        .bind(&event.start_time)
        .bind(&event.end_time)
        .bind(&event.location)
        .bind(&event.scope)
        .bind(&event.category)
        .bind(event.all_day)
        .bind(&event.rrule)
        .bind(event.reminder_minutes_before)
        .bind(&event.created_at)
        .bind(&event.updated_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Event>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, profile_id, title, description, start_time, end_time, location, scope, category, all_day, rrule, reminder_minutes_before, created_at, updated_at
             FROM events WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| Event {
            id: r.get(0),
            profile_id: r.get(1),
            title: r.get(2),
            description: r.get(3),
            start_time: r.get(4),
            end_time: r.get(5),
            location: r.get(6),
            scope: r.get(7),
            category: r.get(8),
            all_day: r.get(9),
            rrule: r.get(10),
            reminder_minutes_before: r.get(11),
            created_at: r.get(12),
            updated_at: r.get(13),
        }))
    }

    /// Delete an event by id. Returns true if an event was deleted, false if not found.
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM events WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_by_category(
        pool: &SqlitePool,
        profile_id: &str,
        category: &str,
    ) -> Result<Vec<Event>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, profile_id, title, description, start_time, end_time, location, scope, category, all_day, rrule, reminder_minutes_before, created_at, updated_at
             FROM events WHERE profile_id = ?1 AND category = ?2",
        )
        .bind(profile_id)
        .bind(category)
        .fetch_all(pool)
        .await?;

        let mut events: Vec<Event> = Vec::new();
        let mut unparseable: Vec<Event> = Vec::new();

        for row in rows {
            let event = event_from_row(&row);
            if DateTime::parse_from_rfc3339(&event.start_time).is_ok() {
                events.push(event);
            } else {
                unparseable.push(event);
            }
        }

        // D9: the same ordering criterion as `list_by_date_range`, by instant.
        events.sort_by(|a, b| {
            match (
                DateTime::parse_from_rfc3339(&a.start_time).ok(),
                DateTime::parse_from_rfc3339(&b.start_time).ok(),
            ) {
                (Some(a_start), Some(b_start)) => a_start.cmp(&b_start),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });

        events.extend(unparseable);

        Ok(events)
    }

    pub async fn list_by_date_range(
        pool: &SqlitePool,
        profile_id: &str,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> Result<Vec<Event>, sqlx::Error> {
        // D1: fetch the profile's rows and decide the time filter in Rust by
        // comparing instants. A SQL text comparison would be wrong for mixed
        // stored formats (`+02:00` vs `Z`) and for bare-date bounds.
        let rows = sqlx::query(
            "SELECT id, profile_id, title, description, start_time, end_time, location, scope, category, all_day, rrule, reminder_minutes_before, created_at, updated_at
             FROM events WHERE profile_id = ?1",
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await?;

        let mut events: Vec<Event> = Vec::new();
        // D8: rows whose timestamps cannot be parsed are included (fail open)
        // but sorted after every parseable row.
        let mut unparseable: Vec<Event> = Vec::new();

        for row in rows {
            let event = event_from_row(&row);
            let event_start = DateTime::parse_from_rfc3339(&event.start_time);
            let event_end = DateTime::parse_from_rfc3339(&event.end_time);
            let (Ok(event_start), Ok(event_end)) = (event_start, event_end) else {
                unparseable.push(event);
                continue;
            };

            if let Some(ref rrule_str) = event.rrule {
                // Recurring base: expand and keep the instances that overlap.
                let instances = expand_recurring(&event_start, &event_end, rrule_str, &start, &end);
                for (inst_start, inst_end) in instances {
                    let mut inst_event = event.clone();
                    inst_event.start_time = inst_start;
                    inst_event.end_time = inst_end;
                    events.push(inst_event);
                }
            } else if event_start < end && event_end > start {
                // D2: semi-open interval [start, end) overlap.
                events.push(event);
            }
        }

        // Order by start instant, not by the stored string (D9). Rows with an
        // unparseable start sort last.
        events.sort_by(|a, b| {
            match (
                DateTime::parse_from_rfc3339(&a.start_time).ok(),
                DateTime::parse_from_rfc3339(&b.start_time).ok(),
            ) {
                (Some(a_start), Some(b_start)) => a_start.cmp(&b_start),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });

        events.extend(unparseable);

        Ok(events)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        pool: &SqlitePool,
        id: &str,
        title: Option<&str>,
        description: Option<&str>,
        location: Option<&str>,
        category: Option<&str>,
        all_day: Option<bool>,
        rrule: Option<&str>,
        reminder_minutes_before: Option<i32>,
        start_time: Option<&str>,
        end_time: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE events SET title = COALESCE(?1, title), description = COALESCE(?2, description), location = COALESCE(?3, location), category = COALESCE(?4, category), all_day = COALESCE(?5, all_day), rrule = COALESCE(?6, rrule), reminder_minutes_before = COALESCE(?7, reminder_minutes_before), start_time = COALESCE(?10, start_time), end_time = COALESCE(?11, end_time), updated_at = ?8 WHERE id = ?9",
        )
        .bind(title)
        .bind(description)
        .bind(location)
        .bind(category)
        .bind(all_day)
        .bind(rrule)
        .bind(reminder_minutes_before)
        .bind(&now)
        .bind(id)
        .bind(start_time)
        .bind(end_time)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn find_free_slots(
        pool: &SqlitePool,
        profile_id: &str,
        date: &str,
        duration_min: i64,
    ) -> Result<Vec<(String, String)>, sqlx::Error> {
        // The day spans [T00:00:00Z of the given date, start of the next day)
        // — semi-open interval, consistent with D2 and with the range queries.
        let day_start = parse_range_bound(date, false).ok_or(sqlx::Error::RowNotFound)?;
        let day_end = day_start + Duration::days(1);

        let events = Self::list_by_date_range(pool, profile_id, day_start, day_end).await?;

        // Compare instants, not strings. Unparseable rows cannot be placed in
        // time, so they are skipped for gap calculation.
        let mut busy: Vec<(DateTime<FixedOffset>, DateTime<FixedOffset>)> = events
            .iter()
            .filter_map(|event| {
                let start = DateTime::parse_from_rfc3339(&event.start_time).ok()?;
                let end = DateTime::parse_from_rfc3339(&event.end_time).ok()?;
                Some((start, end))
            })
            .collect();
        busy.sort_by_key(|a| a.0);

        let mut slots: Vec<(String, String)> = Vec::new();
        let mut cursor = day_start;

        for (event_start, event_end) in busy {
            if cursor < event_start {
                let gap_minutes = (event_start - cursor).num_minutes();
                if gap_minutes >= duration_min {
                    slots.push((fmt_rfc3339(cursor), fmt_rfc3339(event_start)));
                }
            }
            if cursor < event_end {
                cursor = event_end;
            }
        }

        if cursor < day_end {
            let gap_minutes = (day_end - cursor).num_minutes();
            if gap_minutes >= duration_min {
                slots.push((fmt_rfc3339(cursor), fmt_rfc3339(day_end)));
            }
        }

        Ok(slots)
    }
}

/// Parse RRULE strings and generate recurring event instances.
///
/// Supported formats:
/// - `FREQ=DAILY`
/// - `FREQ=WEEKLY;BYDAY=MO,WE,FR`
/// - `FREQ=MONTHLY`
///
/// Returns up to 365 expansions to prevent infinite loops.
/// An instance is returned when its interval overlaps the queried range:
/// `instance_start < range_end && instance_end > range_start`.
pub fn expand_recurring(
    base_start: &DateTime<FixedOffset>,
    base_end: &DateTime<FixedOffset>,
    rrule: &str,
    range_start: &DateTime<FixedOffset>,
    range_end: &DateTime<FixedOffset>,
) -> Vec<(String, String)> {
    let max_expansions: usize = 365;

    let duration = *base_end - *base_start;
    let offset = *base_start.offset();

    // Parse RRULE parts
    let parts: std::collections::HashMap<String, String> = rrule
        .split(';')
        .filter_map(|part| {
            let mut kv = part.splitn(2, '=');
            let key = kv.next()?.to_string();
            let value = kv.next()?.to_string();
            Some((key, value))
        })
        .collect();

    let freq = parts.get("FREQ").map(|s| s.as_str()).unwrap_or("DAILY");
    let byday = parts.get("BYDAY").map(|s| s.as_str()).unwrap_or("");

    // Parse BYDAY into weekdays
    let weekdays: Vec<Weekday> = if byday.is_empty() {
        vec![]
    } else {
        byday
            .split(',')
            .filter_map(|d| match d {
                "MO" => Some(Weekday::Mon),
                "TU" => Some(Weekday::Tue),
                "WE" => Some(Weekday::Wed),
                "TH" => Some(Weekday::Thu),
                "FR" => Some(Weekday::Fri),
                "SA" => Some(Weekday::Sat),
                "SU" => Some(Weekday::Sun),
                _ => None,
            })
            .collect()
    };

    let mut results: Vec<(String, String)> = Vec::new();

    match freq {
        "DAILY" => {
            let mut current = *base_start;
            let mut count = 0;
            while current < *range_end && count < max_expansions {
                let end = current + duration;
                if current < *range_end && end > *range_start {
                    results.push((fmt_rfc3339(current), fmt_rfc3339(end)));
                }
                current += Duration::days(1);
                count += 1;
            }
        }

        "WEEKLY" => {
            if weekdays.is_empty() {
                // Simple weekly: every 7 days from base
                let mut current = *base_start;
                let mut count = 0;
                while current < *range_end && count < max_expansions {
                    let end = current + duration;
                    if current < *range_end && end > *range_start {
                        results.push((fmt_rfc3339(current), fmt_rfc3339(end)));
                    }
                    current += Duration::days(7);
                    count += 1;
                }
            } else {
                // BYDAY: iterate day by day, matching specific weekdays
                let mut current = *base_start;
                let mut count = 0;
                while current < *range_end && count < max_expansions {
                    let end = current + duration;
                    if current < *range_end
                        && end > *range_start
                        && weekdays.contains(&current.weekday())
                    {
                        results.push((fmt_rfc3339(current), fmt_rfc3339(end)));
                    }
                    current += Duration::days(1);
                    count += 1;
                }
            }
        }

        "MONTHLY" => {
            let base_naive = base_start.naive_utc();
            let range_start_naive = range_start.naive_utc();
            let range_end_naive = range_end.naive_utc();

            let mut current_naive = base_naive;
            let mut count = 0;
            while current_naive < range_end_naive && count < max_expansions {
                let end_naive = current_naive + duration;
                if current_naive < range_end_naive && end_naive > range_start_naive {
                    let start_formatted = fmt_rfc3339(
                        DateTime::<FixedOffset>::from_naive_utc_and_offset(current_naive, offset),
                    );
                    let end_formatted = fmt_rfc3339(
                        DateTime::<FixedOffset>::from_naive_utc_and_offset(end_naive, offset),
                    );
                    results.push((start_formatted, end_formatted));
                }
                // Advance one month
                if let Some(next) = add_month_naive(current_naive) {
                    current_naive = next;
                } else {
                    break;
                }
                count += 1;
            }
        }

        _ => {}
    }

    results
}

/// Helper: advance a NaiveDateTime by one calendar month, clamping day to
/// the target month's max.
fn add_month_naive(dt: NaiveDateTime) -> Option<NaiveDateTime> {
    let mut year = dt.year();
    let mut month = dt.month() + 1;
    if month > 12 {
        month = 1;
        year += 1;
    }
    let day = dt.day().min(days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day)
        .map(|d| d.and_hms_opt(dt.hour(), dt.minute(), dt.second()).unwrap())
}

/// Number of days in a given month/year.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        _ => 28,
    }
}

/// Format a DateTime as RFC 3339, always using "Z" for UTC offset.
///
/// The instant is converted to UTC *before* formatting: formatting the wall
/// clock of an offset-bearing value and appending `Z` would re-label the time
/// (e.g. `2026-10-06T19:00:00+02:00` would become `2026-10-06T19:00:00Z`
/// instead of the real instant `2026-10-06T17:00:00Z`).
fn fmt_rfc3339(dt: DateTime<FixedOffset>) -> String {
    dt.with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn setup() -> Result<SqlitePool, sqlx::Error> {
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
        sqlx::query(
            "INSERT INTO profiles (id, name, preferences) VALUES ('profile-1', 'Test', '{}')",
        )
        .execute(&pool)
        .await?;
        Ok(pool)
    }

    fn make_event(id: &str) -> Event {
        Event {
            id: id.into(),
            profile_id: "profile-1".into(),
            title: "Test Event".into(),
            description: None,
            start_time: "2026-09-24T10:00:00Z".into(),
            end_time: "2026-09-24T11:00:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        }
    }

    // ── create / find_by_id ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_event() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let event = Event {
            id: "evt-1".into(),
            profile_id: "profile-1".into(),
            title: "Reunión".into(),
            description: None,
            start_time: "2026-09-24T10:00:00Z".into(),
            end_time: "2026-09-24T11:00:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "meeting".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: Some(15),
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &event).await.unwrap();
        let found = EventsRepo::find_by_id(&pool, "evt-1").await?.unwrap();
        assert_eq!(found.title, "Reunión");
        assert_eq!(found.scope, "shared");
        assert_eq!(found.category, "meeting");
        assert!(!found.all_day);
        assert_eq!(found.rrule, None);
        assert_eq!(found.reminder_minutes_before, Some(15));

        Ok(())
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let found = EventsRepo::find_by_id(&pool, "nonexistent").await.unwrap();
        assert!(found.is_none());

        Ok(())
    }

    // ── delete ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_existing() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let event = make_event("evt-del-1");
        EventsRepo::create(&pool, &event).await.unwrap();

        // Confirm it exists
        assert!(EventsRepo::find_by_id(&pool, "evt-del-1").await?.is_some());

        // Delete
        EventsRepo::delete(&pool, "evt-del-1").await.unwrap();

        // Confirm it's gone
        assert!(EventsRepo::find_by_id(&pool, "evt-del-1").await?.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_nonexistent() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        // Should return false — no event with that id
        assert!(!EventsRepo::delete(&pool, "does-not-exist").await?);
        Ok(())
    }

    // ── list_by_date_range (non-recurring) ─────────────────────────────────

    #[tokio::test]
    async fn test_list_by_date_range() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let event = Event {
            id: "evt-2".into(),
            profile_id: "profile-1".into(),
            title: "Cita".into(),
            description: None,
            start_time: "2026-09-24T15:00:00Z".into(),
            end_time: "2026-09-24T16:00:00Z".into(),
            location: None,
            scope: "personal".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &event).await.unwrap();
        let start = parse_range_bound("2026-09-24T00:00:00Z", false).unwrap();
        let end = parse_range_bound("2026-09-24T23:59:59Z", false).unwrap();
        let events = EventsRepo::list_by_date_range(&pool, "profile-1", start, end).await?;
        assert_eq!(events.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_list_by_date_range_outside() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let event = Event {
            id: "evt-3".into(),
            profile_id: "profile-1".into(),
            title: "Fuera de rango".into(),
            description: None,
            start_time: "2026-09-25T10:00:00Z".into(),
            end_time: "2026-09-25T11:00:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &event).await.unwrap();
        let start = parse_range_bound("2026-09-24T00:00:00Z", false).unwrap();
        let end = parse_range_bound("2026-09-24T23:59:59Z", false).unwrap();
        let events = EventsRepo::list_by_date_range(&pool, "profile-1", start, end).await?;
        assert!(events.is_empty());

        Ok(())
    }

    // ── list_by_date_range with recurring events ──────────────────────────

    #[tokio::test]
    async fn test_list_by_date_range_with_recurring() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;

        // Non-recurring event in range
        let non_rec = Event {
            id: "evt-normal".into(),
            profile_id: "profile-1".into(),
            title: "Normal".into(),
            description: None,
            start_time: "2026-09-24T10:00:00Z".into(),
            end_time: "2026-09-24T11:00:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &non_rec).await.unwrap();

        // Recurring daily event starting 2026-09-20
        let recurring = Event {
            id: "evt-daily".into(),
            profile_id: "profile-1".into(),
            title: "Daily Standup".into(),
            description: None,
            start_time: "2026-09-20T09:00:00Z".into(),
            end_time: "2026-09-20T09:30:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: Some("FREQ=DAILY".into()),
            reminder_minutes_before: None,
            created_at: "2026-09-19T00:00:00Z".into(),
            updated_at: "2026-09-19T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &recurring).await.unwrap();

        // Query range: 2026-09-24
        let start = parse_range_bound("2026-09-24T00:00:00Z", false).unwrap();
        let end = parse_range_bound("2026-09-24T23:59:59Z", false).unwrap();
        let events = EventsRepo::list_by_date_range(&pool, "profile-1", start, end).await?;

        // Should include Normal (10:00-11:00) + Daily Standup (09:00-09:30)
        assert_eq!(events.len(), 2, "should have normal + recurring expansion");
        // Sorted: Daily Standup at 09:00, then Normal at 10:00
        assert_eq!(events[0].title, "Daily Standup");
        assert_eq!(events[1].title, "Normal");

        Ok(())
    }

    // ── list_by_category ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_by_category() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;

        let e1 = Event {
            id: "evt-cat-1".into(),
            profile_id: "profile-1".into(),
            title: "Meeting".into(),
            description: None,
            start_time: "2026-09-24T10:00:00Z".into(),
            end_time: "2026-09-24T11:00:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "meeting".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        let e2 = Event {
            id: "evt-cat-2".into(),
            profile_id: "profile-1".into(),
            title: "Birthday".into(),
            description: None,
            start_time: "2026-09-25T10:00:00Z".into(),
            end_time: "2026-09-25T11:00:00Z".into(),
            location: None,
            scope: "personal".into(),
            category: "birthday".into(),
            all_day: true,
            rrule: Some("FREQ=YEARLY".into()),
            reminder_minutes_before: Some(1440),
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &e1).await.unwrap();
        EventsRepo::create(&pool, &e2).await.unwrap();

        let meetings = EventsRepo::list_by_category(&pool, "profile-1", "meeting").await?;
        assert_eq!(meetings.len(), 1);
        assert_eq!(meetings[0].title, "Meeting");

        let birthdays = EventsRepo::list_by_category(&pool, "profile-1", "birthday").await?;
        assert_eq!(birthdays.len(), 1);
        assert_eq!(birthdays[0].title, "Birthday");
        assert!(birthdays[0].all_day);
        assert_eq!(birthdays[0].rrule.as_deref(), Some("FREQ=YEARLY"));
        assert_eq!(birthdays[0].reminder_minutes_before, Some(1440));

        let empty = EventsRepo::list_by_category(&pool, "profile-1", "nonexistent").await?;
        assert!(empty.is_empty());

        Ok(())
    }

    // ── update ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_event() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let event = Event {
            id: "evt-4".into(),
            profile_id: "profile-1".into(),
            title: "Original".into(),
            description: None,
            start_time: "2026-09-24T10:00:00Z".into(),
            end_time: "2026-09-24T11:00:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &event).await.unwrap();
        EventsRepo::update(
            &pool,
            "evt-4",
            Some("Actualizado"),
            None,
            Some("Oficina"),
            None,
            None,
            None,
            None,
            Some("2026-09-25T14:00:00Z"),
            Some("2026-09-25T15:00:00Z"),
        )
        .await?;
        let found = EventsRepo::find_by_id(&pool, "evt-4").await?.unwrap();
        assert_eq!(found.title, "Actualizado");
        assert_eq!(found.location.unwrap(), "Oficina");
        assert_eq!(found.start_time, "2026-09-25T14:00:00Z");
        assert_eq!(found.end_time, "2026-09-25T15:00:00Z");

        Ok(())
    }

    // ── find_free_slots ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_free_slots() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let event = Event {
            id: "evt-5".into(),
            profile_id: "profile-1".into(),
            title: "Ocupado".into(),
            description: None,
            start_time: "2026-09-24T10:00:00Z".into(),
            end_time: "2026-09-24T11:00:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "".into(),
            updated_at: "".into(),
        };
        EventsRepo::create(&pool, &event).await.unwrap();
        let slots = EventsRepo::find_free_slots(&pool, "profile-1", "2026-09-24", 30).await?;
        assert!(!slots.is_empty());
        // Should have a slot before 10:00 and after 11:00
        assert!(slots.len() >= 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_find_free_slots_mixed_formats() -> Result<(), Box<dyn std::error::Error>> {
        // §5.1 — an event stored with `+02:00` and another with `Z` must be
        // ordered and clipped by instant, not by string comparison.
        let pool = setup().await?;
        // 19:00+02:00 == 17:00Z .. 20:00+02:00 == 18:00Z
        let offset_event = Event {
            id: "evt-offset".into(),
            profile_id: "profile-1".into(),
            title: "Con offset".into(),
            description: None,
            start_time: "2026-10-06T19:00:00+02:00".into(),
            end_time: "2026-10-06T20:00:00+02:00".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        // 18:30Z .. 19:30Z
        let utc_event = Event {
            id: "evt-utc".into(),
            profile_id: "profile-1".into(),
            title: "Con Z".into(),
            description: None,
            start_time: "2026-10-06T18:30:00Z".into(),
            end_time: "2026-10-06T19:30:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &offset_event).await.unwrap();
        EventsRepo::create(&pool, &utc_event).await.unwrap();

        let slots = EventsRepo::find_free_slots(&pool, "profile-1", "2026-10-06", 30).await?;

        // The first gap runs from the start of the day to the earliest event
        // start by instant: 17:00Z (`19:00+02:00`), which sorts *before*
        // `18:30Z` even though its stored string is lexicographically later.
        assert!(
            slots.contains(&(
                "2026-10-06T00:00:00Z".to_string(),
                "2026-10-06T17:00:00Z".to_string()
            )),
            "the first gap must end at the earliest instant, got {slots:?}"
        );
        // No gap may overlap either event: after 19:30Z nothing starts until 20:00Z.
        for (gap_start, gap_end) in &slots {
            let gs = DateTime::parse_from_rfc3339(gap_start).unwrap();
            let ge = DateTime::parse_from_rfc3339(gap_end).unwrap();
            let overlaps_a = gs < DateTime::parse_from_rfc3339("2026-10-06T18:00:00Z").unwrap()
                && ge > DateTime::parse_from_rfc3339("2026-10-06T17:00:00Z").unwrap();
            let overlaps_b = gs < DateTime::parse_from_rfc3339("2026-10-06T19:30:00Z").unwrap()
                && ge > DateTime::parse_from_rfc3339("2026-10-06T18:30:00Z").unwrap();
            assert!(
                !overlaps_a,
                "gap {gap_start}..{gap_end} overlaps the offset event"
            );
            assert!(
                !overlaps_b,
                "gap {gap_start}..{gap_end} overlaps the Z event"
            );
        }
        Ok(())
    }

    // ── expand_recurring unit tests ────────────────────────────────────────

    #[test]
    fn test_expand_recurring_daily() {
        let base_start = DateTime::parse_from_rfc3339("2026-09-24T10:00:00Z").unwrap();
        let base_end = DateTime::parse_from_rfc3339("2026-09-24T11:00:00Z").unwrap();
        let range_start = DateTime::parse_from_rfc3339("2026-09-25T00:00:00Z").unwrap();
        let range_end = DateTime::parse_from_rfc3339("2026-09-28T00:00:00Z").unwrap();
        let results = expand_recurring(
            &base_start,
            &base_end,
            "FREQ=DAILY",
            &range_start,
            &range_end,
        );

        // Should have instances on Sep 25, 26, 27 (3 days within range)
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, "2026-09-25T10:00:00Z");
        assert_eq!(results[0].1, "2026-09-25T11:00:00Z");
        assert_eq!(results[1].0, "2026-09-26T10:00:00Z");
        assert_eq!(results[2].0, "2026-09-27T10:00:00Z");
    }

    #[test]
    fn test_expand_recurring_weekly() {
        // Base: Monday 2026-09-21 (a Monday)
        // Recur Mon/Wed/Fri
        let base_start = DateTime::parse_from_rfc3339("2026-09-21T09:00:00Z").unwrap();
        let base_end = DateTime::parse_from_rfc3339("2026-09-21T10:00:00Z").unwrap();
        let range_start = DateTime::parse_from_rfc3339("2026-09-21T00:00:00Z").unwrap();
        let range_end = DateTime::parse_from_rfc3339("2026-09-28T00:00:00Z").unwrap();
        let results = expand_recurring(
            &base_start,
            &base_end,
            "FREQ=WEEKLY;BYDAY=MO,WE,FR",
            &range_start,
            &range_end,
        );

        // Week of Sep 21: Mon 21, Wed 23, Fri 25 = 3 instances
        // Week of Sep 28 starts Sep 28 (Monday) which is at the boundary
        // Range is [Sep 21, Sep 28), so Sep 28 is excluded
        assert_eq!(results.len(), 3, "expected Mon 21, Wed 23, Fri 25");
        assert_eq!(results[0].0, "2026-09-21T09:00:00Z");
        assert_eq!(results[1].0, "2026-09-23T09:00:00Z");
        assert_eq!(results[2].0, "2026-09-25T09:00:00Z");
    }

    #[test]
    fn test_expand_recurring_monthly() {
        let base_start = DateTime::parse_from_rfc3339("2026-01-15T10:00:00Z").unwrap();
        let base_end = DateTime::parse_from_rfc3339("2026-01-15T11:00:00Z").unwrap();
        let range_start = DateTime::parse_from_rfc3339("2026-03-01T00:00:00Z").unwrap();
        let range_end = DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z").unwrap();
        let results = expand_recurring(
            &base_start,
            &base_end,
            "FREQ=MONTHLY",
            &range_start,
            &range_end,
        );

        // Mar 15, Apr 15, May 15 = 3 instances
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, "2026-03-15T10:00:00Z");
        assert_eq!(results[1].0, "2026-04-15T10:00:00Z");
        assert_eq!(results[2].0, "2026-05-15T10:00:00Z");
    }

    #[test]
    fn test_expand_recurring_outside_range() {
        let base_start = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z").unwrap();
        let base_end = DateTime::parse_from_rfc3339("2026-01-01T11:00:00Z").unwrap();
        let range_start = DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z").unwrap();
        let range_end = DateTime::parse_from_rfc3339("2026-06-05T00:00:00Z").unwrap();
        let results = expand_recurring(
            &base_start,
            &base_end,
            "FREQ=DAILY",
            &range_start,
            &range_end,
        );

        // Jan 1 daily, but range starts Jun 1 — should expand from Jan 1
        // forward until we reach the range, producing up to 365 results
        // The first result should be Jun 1
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "2026-06-01T10:00:00Z");
        assert_eq!(results.last().unwrap().0, "2026-06-04T10:00:00Z");
    }

    #[test]
    fn test_expand_recurring_invalid_rrule() {
        let base_start = DateTime::parse_from_rfc3339("2026-09-24T10:00:00Z").unwrap();
        let base_end = DateTime::parse_from_rfc3339("2026-09-24T11:00:00Z").unwrap();
        let range_start = DateTime::parse_from_rfc3339("2026-09-24T00:00:00Z").unwrap();
        let range_end = DateTime::parse_from_rfc3339("2026-09-25T00:00:00Z").unwrap();
        let results = expand_recurring(
            &base_start,
            &base_end,
            "FREQ=YEARLY",
            &range_start,
            &range_end,
        );
        // YEARLY is not supported — returns empty
        assert!(results.is_empty());
    }

    #[test]
    fn test_expand_recurring_max_365() {
        let base_start = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap();
        let base_end = DateTime::parse_from_rfc3339("2026-01-01T01:00:00Z").unwrap();
        let range_start = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap();
        let range_end = DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z").unwrap();
        let results = expand_recurring(
            &base_start,
            &base_end,
            "FREQ=DAILY",
            &range_start,
            &range_end,
        );
        // Should cap at 365
        assert_eq!(results.len(), 365);
    }

    #[test]
    fn test_expand_recurring_bad_dates() {
        // Base instants are now parsed at the edge; an unsupported RRULE must
        // still yield no instances (never panic, never invent dates).
        let base_start = DateTime::parse_from_rfc3339("2026-09-24T10:00:00Z").unwrap();
        let base_end = DateTime::parse_from_rfc3339("2026-09-24T11:00:00Z").unwrap();
        let range_start = DateTime::parse_from_rfc3339("2026-09-25T00:00:00Z").unwrap();
        let range_end = DateTime::parse_from_rfc3339("2026-09-28T00:00:00Z").unwrap();
        let results = expand_recurring(
            &base_start,
            &base_end,
            "FREQ=YEARLY",
            &range_start,
            &range_end,
        );
        assert!(results.is_empty());
    }

    #[test]
    fn test_expand_recurring_partial_overlap() {
        // §3.2 — an instance that starts before the range lower bound and ends
        // inside it SHALL be included (overlap, not "starts within").
        let base_start = DateTime::parse_from_rfc3339("2026-09-26T23:00:00Z").unwrap();
        let base_end = DateTime::parse_from_rfc3339("2026-09-27T01:00:00Z").unwrap();
        let range_start = DateTime::parse_from_rfc3339("2026-09-27T00:00:00Z").unwrap();
        let range_end = DateTime::parse_from_rfc3339("2026-09-28T00:00:00Z").unwrap();
        let results = expand_recurring(
            &base_start,
            &base_end,
            "FREQ=DAILY",
            &range_start,
            &range_end,
        );

        assert!(
            results
                .iter()
                .any(|(s, e)| s == "2026-09-26T23:00:00Z" && e == "2026-09-27T01:00:00Z"),
            "the instance overlapping the lower bound must be included, got {results:?}"
        );
    }

    #[test]
    fn test_expand_recurring_emits_canonical_utc() {
        // §3.4 — base stored with a `+02:00` offset must be emitted in UTC
        // canonical form without re-labeling the wall clock.
        let base_start = DateTime::parse_from_rfc3339("2026-10-06T19:00:00+02:00").unwrap();
        let base_end = DateTime::parse_from_rfc3339("2026-10-06T20:00:00+02:00").unwrap();
        let range_start = DateTime::parse_from_rfc3339("2026-10-06T00:00:00Z").unwrap();
        let range_end = DateTime::parse_from_rfc3339("2026-10-07T00:00:00Z").unwrap();
        let results = expand_recurring(
            &base_start,
            &base_end,
            "FREQ=DAILY",
            &range_start,
            &range_end,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "2026-10-06T17:00:00Z");
        assert_eq!(results[0].1, "2026-10-06T18:00:00Z");
    }

    #[test]
    fn test_expand_recurring_base_far_before_range_returns_nothing() {
        // Scenario: "Recurring event outside range returns nothing" — a daily
        // base more than the 365-expansion cap before the queried range cannot
        // reach it, so no instance is emitted.
        let base_start = DateTime::parse_from_rfc3339("2025-10-20T09:00:00Z").unwrap();
        let base_end = DateTime::parse_from_rfc3339("2025-10-20T10:00:00Z").unwrap();
        let range_start = DateTime::parse_from_rfc3339("2026-11-02T00:00:00Z").unwrap();
        let range_end = DateTime::parse_from_rfc3339("2026-11-09T00:00:00Z").unwrap();
        let results = expand_recurring(
            &base_start,
            &base_end,
            "FREQ=DAILY",
            &range_start,
            &range_end,
        );

        assert!(
            results.is_empty(),
            "a daily base beyond the expansion cap must not reach next week, got {results:?}"
        );
    }

    // ── parse_range_bound unit tests ───────────────────────────────────────

    #[test]
    fn test_parse_range_bound_rfc3339_z() {
        let result = parse_range_bound("2026-10-31T09:00:00Z", false).unwrap();
        assert_eq!(result.to_rfc3339(), "2026-10-31T09:00:00+00:00");
    }

    #[test]
    fn test_parse_range_bound_rfc3339_offset() {
        let result = parse_range_bound("2026-10-31T09:00:00+02:00", false).unwrap();
        assert_eq!(result.to_rfc3339(), "2026-10-31T09:00:00+02:00");
    }

    #[test]
    fn test_parse_range_bound_date_only_lower() {
        let result = parse_range_bound("2026-10-31", false).unwrap();
        assert_eq!(result.to_rfc3339(), "2026-10-31T00:00:00+00:00");
    }

    #[test]
    fn test_parse_range_bound_date_only_upper() {
        let result = parse_range_bound("2026-10-31", true).unwrap();
        assert_eq!(result.to_rfc3339(), "2026-11-01T00:00:00+00:00");
    }

    #[test]
    fn test_parse_range_bound_invalid() {
        assert!(parse_range_bound("no-es-una-fecha", false).is_none());
    }

    // ── list_by_date_range: nuevo comportamiento (instantes) ───────────────

    #[tokio::test]
    async fn test_list_by_date_range_includes_last_day() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let event = Event {
            id: "evt-last-day".into(),
            profile_id: "profile-1".into(),
            title: "Last Day Event".into(),
            description: None,
            start_time: "2026-10-31T08:00:00Z".into(),
            end_time: "2026-10-31T09:00:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &event).await.unwrap();

        let start = parse_range_bound("2026-09-27", false).unwrap();
        let end = parse_range_bound("2026-10-31", true).unwrap();
        let events = EventsRepo::list_by_date_range(&pool, "profile-1", start, end).await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Last Day Event");
        Ok(())
    }

    #[tokio::test]
    async fn test_list_by_date_range_overlap_start() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let event = Event {
            id: "evt-overlap-start".into(),
            profile_id: "profile-1".into(),
            title: "Overlap Start".into(),
            description: None,
            start_time: "2026-09-26T23:00:00Z".into(),
            end_time: "2026-09-27T01:00:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &event).await.unwrap();

        let start = parse_range_bound("2026-09-27T00:00:00Z", false).unwrap();
        let end = parse_range_bound("2026-09-28T00:00:00Z", false).unwrap();
        let events = EventsRepo::list_by_date_range(&pool, "profile-1", start, end).await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Overlap Start");
        Ok(())
    }

    #[tokio::test]
    async fn test_list_by_date_range_semi_open_boundary() -> Result<(), Box<dyn std::error::Error>>
    {
        let pool = setup().await?;
        let event = Event {
            id: "evt-boundary".into(),
            profile_id: "profile-1".into(),
            title: "Boundary Event".into(),
            description: None,
            start_time: "2026-09-27T00:00:00Z".into(),
            end_time: "2026-09-27T01:00:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &event).await.unwrap();

        // Event ends exactly at the lower bound -> excluded
        let start = parse_range_bound("2026-09-27T01:00:00Z", false).unwrap();
        let end = parse_range_bound("2026-09-28T00:00:00Z", false).unwrap();
        let events = EventsRepo::list_by_date_range(&pool, "profile-1", start, end).await?;
        assert!(events.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_list_by_date_range_mixed_formats() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let event_a = Event {
            id: "evt-a".into(),
            profile_id: "profile-1".into(),
            title: "Event A".into(),
            description: None,
            start_time: "2026-10-06T19:00:00+02:00".into(),
            end_time: "2026-10-06T20:00:00+02:00".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        let event_b = Event {
            id: "evt-b".into(),
            profile_id: "profile-1".into(),
            title: "Event B".into(),
            description: None,
            start_time: "2026-10-06T18:30:00Z".into(),
            end_time: "2026-10-06T19:30:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &event_a).await.unwrap();
        EventsRepo::create(&pool, &event_b).await.unwrap();

        // Range [18:15Z, 18:45Z] -> B overlaps, A does not
        let start = parse_range_bound("2026-10-06T18:15:00Z", false).unwrap();
        let end = parse_range_bound("2026-10-06T18:45:00Z", false).unwrap();
        let events = EventsRepo::list_by_date_range(&pool, "profile-1", start, end).await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Event B");
        Ok(())
    }

    #[tokio::test]
    async fn test_list_by_date_range_order_by_instant() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let event_a = Event {
            id: "evt-order-a".into(),
            profile_id: "profile-1".into(),
            title: "A".into(),
            description: None,
            start_time: "2026-10-06T19:00:00+02:00".into(),
            end_time: "2026-10-06T20:00:00+02:00".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        let event_b = Event {
            id: "evt-order-b".into(),
            profile_id: "profile-1".into(),
            title: "B".into(),
            description: None,
            start_time: "2026-10-06T18:30:00Z".into(),
            end_time: "2026-10-06T19:30:00Z".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &event_a).await.unwrap();
        EventsRepo::create(&pool, &event_b).await.unwrap();

        let start = parse_range_bound("2026-10-06T00:00:00Z", false).unwrap();
        let end = parse_range_bound("2026-10-07T00:00:00Z", false).unwrap();
        let events = EventsRepo::list_by_date_range(&pool, "profile-1", start, end).await?;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].title, "A");
        assert_eq!(events[1].title, "B");
        Ok(())
    }

    #[tokio::test]
    async fn test_list_by_date_range_unparseable_last() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let event = Event {
            id: "evt-unparseable".into(),
            profile_id: "profile-1".into(),
            title: "Unparseable".into(),
            description: None,
            start_time: "not-a-date".into(),
            end_time: "not-a-date".into(),
            location: None,
            scope: "shared".into(),
            category: "default".into(),
            all_day: false,
            rrule: None,
            reminder_minutes_before: None,
            created_at: "2026-09-23T00:00:00Z".into(),
            updated_at: "2026-09-23T00:00:00Z".into(),
        };
        EventsRepo::create(&pool, &event).await.unwrap();

        let start = parse_range_bound("2026-10-06T00:00:00Z", false).unwrap();
        let end = parse_range_bound("2026-10-07T00:00:00Z", false).unwrap();
        let events = EventsRepo::list_by_date_range(&pool, "profile-1", start, end).await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Unparseable");
        Ok(())
    }
}
