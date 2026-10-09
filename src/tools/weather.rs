use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde_json::Value;
use sqlx::SqlitePool;
use std::time::Duration;

use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Default OpenWeather API base URL.
const DEFAULT_WEATHER_BASE_URL: &str = "https://api.openweathermap.org/data/2.5";

/// Parse a date string, accepting both ISO 8601 datetime and date-only YYYY-MM-DD formats.
///
/// - `"2026-09-25T12:00:00Z"` → parsed directly as `DateTime<Utc>`
/// - `"2026-09-25"` → interpreted as noon UTC on that day
/// - Any other format → returns `None`
fn parse_forecast_date(date_str: &str) -> Option<DateTime<Utc>> {
    date_str.parse::<DateTime<Utc>>().ok().or_else(|| {
        // Try parsing as YYYY-MM-DD (date only), defaulting to noon UTC
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(12, 0, 0))
            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
    })
}

pub struct WeatherTool {
    #[allow(dead_code)]
    db: SqlitePool,
    api_key: String,
    base_url: String,
    timeout: Option<Duration>,
}

impl WeatherTool {
    /// Build a `reqwest::Client`, applying an explicit timeout when one is
    /// provided. `None` (used only by tests) leaves reqwest's default, i.e. no
    /// timeout.
    fn build_client(timeout: Option<Duration>) -> reqwest::Client {
        match timeout {
            Some(d) => reqwest::Client::builder()
                .timeout(d)
                .build()
                .expect("Failed to create HTTP client"),
            None => reqwest::Client::new(),
        }
    }

    pub fn new(db: SqlitePool, api_key: String) -> Self {
        Self {
            db,
            api_key,
            base_url: DEFAULT_WEATHER_BASE_URL.to_string(),
            timeout: Some(Duration::from_secs(30)),
        }
    }

    /// Test-only constructor allowing a custom base URL and timeout.
    #[cfg(test)]
    pub fn with_base_url_and_timeout(
        db: SqlitePool,
        api_key: String,
        base_url: String,
        timeout: Option<Duration>,
    ) -> Self {
        Self {
            db,
            api_key,
            base_url,
            timeout,
        }
    }

    /// Test-only accessor for the configured HTTP timeout.
    #[cfg(test)]
    pub fn http_timeout(&self) -> Option<Duration> {
        self.timeout
    }

    async fn get_current_weather(
        &self,
        lat: f64,
        lon: f64,
        api_key: &str,
    ) -> Result<ToolResult, ToolError> {
        let client = Self::build_client(self.timeout);
        let url = format!(
            "{}/weather?lat={}&lon={}&appid={}&units=metric&lang=es",
            self.base_url, lat, lon, api_key
        );

        let resp = client.get(&url).send().await.map_err(|e| {
            ToolError::ExecutionError(format!("Failed to call OpenWeather API: {}", e))
        })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionError(format!(
                "OpenWeather API returned {}: {}",
                status, body
            )));
        }

        let json: Value = resp.json().await.map_err(|e| {
            ToolError::ExecutionError(format!("Failed to parse OpenWeather response: {}", e))
        })?;

        Ok(ToolResult {
            success: true,
            data: json,
            message: None,
        })
    }

    async fn get_forecast(
        &self,
        lat: f64,
        lon: f64,
        date_str: &str,
        api_key: &str,
    ) -> Result<ToolResult, ToolError> {
        let client = Self::build_client(self.timeout);
        let url = format!(
            "{}/forecast?lat={}&lon={}&appid={}&units=metric&lang=es",
            self.base_url, lat, lon, api_key
        );

        let resp = client.get(&url).send().await.map_err(|e| {
            ToolError::ExecutionError(format!("Failed to call OpenWeather API: {}", e))
        })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionError(format!(
                "OpenWeather API returned {}: {}",
                status, body
            )));
        }

        let json: Value = resp.json().await.map_err(|e| {
            ToolError::ExecutionError(format!("Failed to parse OpenWeather response: {}", e))
        })?;

        let target_date = parse_forecast_date(date_str);

        let list = json
            .get("list")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ToolError::ExecutionError("No forecast data in response".into()))?;

        // Find the forecast entry closest to the target date
        let target = target_date.ok_or_else(|| {
            ToolError::ExecutionError(format!(
                "Invalid date format: '{}'. Use YYYY-MM-DD or ISO 8601.",
                date_str
            ))
        })?;

        let closest = list
            .iter()
            .filter_map(|entry| {
                let dt = entry.get("dt_txt").and_then(|v| v.as_str()).and_then(|s| {
                    // OpenWeather dt_txt format: "2026-09-28 00:00:00" (space-separated, no tz)
                    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                        .ok()
                        .map(|ndt| DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
                        // Fallback for ISO 8601 format
                        .or_else(|| s.parse::<DateTime<Utc>>().ok())
                })?;
                let diff = (dt - target).num_seconds().abs();
                Some((diff, entry))
            })
            .min_by_key(|(diff, _)| *diff)
            .map(|(_, entry)| entry);

        match closest {
            Some(entry) => Ok(ToolResult {
                success: true,
                data: entry.clone(),
                message: None,
            }),
            None => Err(ToolError::ExecutionError(
                "No forecast entry found for the specified date".into(),
            )),
        }
    }
}

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &'static str {
        "weather"
    }

    fn description(&self) -> &'static str {
        "Consulta del clima actual o pronóstico para coordenadas geográficas. Si solo tienes el nombre de la ciudad, usa primero `geocode` para obtener las coordenadas."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "latitude": {
                    "type": "number",
                    "description": "Latitud en grados decimales"
                },
                "longitude": {
                    "type": "number",
                    "description": "Longitud en grados decimales"
                },
                "date": {
                    "type": "string",
                    "description": "Fecha opcional para pronóstico (formato ISO 8601 o YYYY-MM-DD)"
                }
            },
            "required": ["latitude", "longitude"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        // Try to get API key from settings DB first, fallback to Config/ENV
        let api_key =
            match crate::db::repos::settings::SettingsRepo::get(&self.db, "openweather_api_key")
                .await
            {
                Ok(Some(key)) if !key.is_empty() => key,
                _ => self.api_key.clone(),
            };

        if api_key.is_empty() {
            return Err(ToolError::ExecutionError(
                "OpenWeather API key is not configured".into(),
            ));
        }

        let lat = args
            .get("latitude")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ToolError::InvalidArguments("Missing or invalid latitude".into()))?;
        let lon = args
            .get("longitude")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ToolError::InvalidArguments("Missing or invalid longitude".into()))?;

        let date = args.get("date").and_then(|v| v.as_str());

        if let Some(date_str) = date {
            self.get_forecast(lat, lon, date_str, &api_key).await
        } else {
            self.get_current_weather(lat, lon, &api_key).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup() -> Result<(SqlitePool, WeatherTool), sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        run_migrations(&pool).await.unwrap();
        let tool = WeatherTool::new(pool.clone(), "test-api-key".into());
        Ok((pool, tool))
    }

    #[tokio::test]
    async fn test_weather_name_and_description() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        assert_eq!(tool.name(), "weather");
        assert!(
            tool.description().contains("geocode"),
            "weather description must guide the model to `geocode` when only a city name is known, got: {}",
            tool.description()
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_weather_permission() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        assert_eq!(
            tool.permission(&serde_json::json!({})),
            Permission::NoConfirm
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_weather_parameters_has_no_operation() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        // Must NOT have an operation property
        assert!(
            params["properties"].get("operation").is_none(),
            "Should not have operation property"
        );
        // Required must be latitude and longitude
        let required = params["required"].as_array().unwrap();
        let req_values: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(req_values, vec!["latitude", "longitude"]);
        // Properties should have latitude, longitude, date
        let props = params["properties"].as_object().unwrap();
        assert!(props.contains_key("latitude"), "Should have latitude");
        assert!(props.contains_key("longitude"), "Should have longitude");
        assert!(props.contains_key("date"), "Should have date");
        Ok(())
    }

    #[tokio::test]
    async fn test_weather_missing_latitude_returns_error() -> Result<(), Box<dyn std::error::Error>>
    {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "longitude": -3.7038
            }))
            .await;
        assert!(
            matches!(result, Err(ToolError::InvalidArguments(_))),
            "Expected InvalidArguments error for missing latitude"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_weather_missing_longitude_returns_error() -> Result<(), Box<dyn std::error::Error>>
    {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "latitude": 40.4168
            }))
            .await;
        assert!(
            matches!(result, Err(ToolError::InvalidArguments(_))),
            "Expected InvalidArguments error for missing longitude"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_weather_missing_both_coordinates() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool.execute(serde_json::json!({})).await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
        Ok(())
    }

    // ------------------------------------------------------------------
    // parse_forecast_date tests
    // ------------------------------------------------------------------

    #[test]
    fn test_parse_forecast_date_full_datetime() {
        let dt = parse_forecast_date("2026-09-25T12:00:00Z");
        assert!(dt.is_some(), "Full ISO 8601 datetime should parse");
        assert_eq!(dt.unwrap().to_rfc3339(), "2026-09-25T12:00:00+00:00");
    }

    #[test]
    fn test_parse_forecast_date_only_format() {
        // "2026-09-25" should parse via NaiveDate fallback (noon UTC)
        let dt = parse_forecast_date("2026-09-25");
        assert!(dt.is_some(), "Date-only YYYY-MM-DD should parse");
        assert_eq!(dt.unwrap().to_rfc3339(), "2026-09-25T12:00:00+00:00");
    }

    #[test]
    fn test_parse_forecast_invalid_date_returns_none() {
        let dt = parse_forecast_date("not-a-date");
        assert!(dt.is_none(), "Invalid date string should return None");

        let dt = parse_forecast_date("");
        assert!(dt.is_none(), "Empty string should return None");
    }

    #[test]
    fn test_parse_forecast_alt_format() {
        // "2026-09-25 00:00:00" (common in forecast dt_txt) should NOT parse
        // because it's not ISO 8601 and not YYYY-MM-DD
        let dt = parse_forecast_date("2026-09-25 00:00:00");
        assert!(dt.is_none(), "Space-separated datetime should not parse");
    }

    #[test]
    fn test_parse_forecast_edge_date() {
        // Leap year date
        let dt = parse_forecast_date("2024-02-29");
        assert!(dt.is_some(), "Leap year date should parse");
        assert_eq!(dt.unwrap().to_rfc3339(), "2024-02-29T12:00:00+00:00");
    }

    // ------------------------------------------------------------------
    // RED phase — HTTP timeout
    // ------------------------------------------------------------------

    /// RED: `new` must configure an explicit 30 s timeout on its HTTP client.
    /// Currently `new` leaves `timeout = None`, so this assertion fails.
    #[tokio::test]
    async fn test_weather_new_sets_default_http_timeout() -> Result<(), Box<dyn std::error::Error>>
    {
        let (_, tool) = setup().await?;
        assert_eq!(
            tool.http_timeout(),
            Some(Duration::from_secs(30)),
            "WeatherTool::new must configure an explicit 30 s HTTP timeout"
        );
        Ok(())
    }

    /// Guard: when a timeout is explicitly configured, a slow server must make
    /// `execute` return an `ExecutionError` mentioning OpenWeather instead of
    /// hanging. This confirms the timeout mechanism works end-to-end.
    #[tokio::test]
    async fn test_weather_execute_times_out_on_slow_server() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/weather"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(500))
                    .set_body_json(serde_json::json!({})),
            )
            .mount(&server)
            .await;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        run_migrations(&pool).await.unwrap();

        let tool = WeatherTool::with_base_url_and_timeout(
            pool,
            "test-key".into(),
            server.uri(),
            Some(Duration::from_millis(50)),
        );

        let result = tool
            .execute(serde_json::json!({"latitude": 40.4168, "longitude": -3.7038}))
            .await;

        match result {
            Err(ToolError::ExecutionError(msg)) => {
                assert!(
                    msg.contains("OpenWeather"),
                    "timeout error should mention OpenWeather, got: {msg}"
                );
            }
            other => panic!("expected Err(ToolError::ExecutionError), got: {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // RED — improve-tool-schemas: the weather description must guide the model
    // to resolve a city name with `geocode` first.
    // -----------------------------------------------------------------------

    /// Scenario: La descripción de weather guía a geocode
    #[tokio::test]
    async fn test_weather_description_mentions_geocode() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let desc = tool.description();
        assert!(
            desc.contains("geocode"),
            "weather description must mention `geocode` for city names, got: {desc}"
        );
        Ok(())
    }
}
