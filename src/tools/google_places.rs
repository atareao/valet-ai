//! Google Places New API client and `SearchPlacesTool`.
//!
//! Replaces the Overpass API-based search_places with a Google Places
//! New API implementation. The API key is read from the settings database
//! at runtime (key: `google_places_api_key`).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;

use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Default base URL for the Google Places New API.
const DEFAULT_PLACES_BASE_URL: &str = "https://places.googleapis.com/v1";

// ---------------------------------------------------------------------------
// Data types — mirrors the Google Places New API response shape
// ---------------------------------------------------------------------------

/// A single place returned by the Google Places New API.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    pub id: Option<String>,
    pub display_name: Option<LocalizedText>,
    pub formatted_address: Option<String>,
    pub location: Option<LatLng>,
    pub rating: Option<f64>,
    pub user_rating_count: Option<u64>,
    pub price_level: Option<String>,
    pub website_uri: Option<String>,
    pub national_phone_number: Option<String>,
    pub regular_opening_hours: Option<OpeningHours>,
    pub primary_type: Option<String>,
    pub types: Option<Vec<String>>,
    pub editorial_summary: Option<LocalizedText>,
}

/// Localised text (language code + content).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedText {
    pub text: Option<String>,
    pub language_code: Option<String>,
}

/// Geographic coordinates.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LatLng {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

/// Regular opening hours for a place.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpeningHours {
    pub open_now: Option<bool>,
    pub weekday_descriptions: Option<Vec<String>>,
}

/// Top-level response from the Google Places API.
#[derive(Debug, Deserialize)]
pub struct PlacesResponse {
    pub places: Option<Vec<Place>>,
    #[serde(default)]
    pub error: Option<PlacesError>,
}

/// Error payload optionally returned by the Google Places API.
#[derive(Debug, Deserialize)]
pub struct PlacesError {
    pub message: Option<String>,
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------
// Field mask constant — sent in every request to control cost & payload size
// ---------------------------------------------------------------------------

/// Comma-separated list of fields to request from the Places API.
/// Minimises payload size and API cost (billing is per-field).
pub const FIELD_MASK: &str = concat!(
    "places.id,",
    "places.displayName,",
    "places.formattedAddress,",
    "places.location,",
    "places.rating,",
    "places.userRatingCount,",
    "places.priceLevel,",
    "places.websiteUri,",
    "places.nationalPhoneNumber,",
    "places.regularOpeningHours,",
    "places.primaryType,",
    "places.types,",
    "places.editorialSummary",
);

// ---------------------------------------------------------------------------
// HTTP client for the Google Places New API
// ---------------------------------------------------------------------------

/// Low-level HTTP client for the Google Places New API.
///
/// Wraps `reqwest::Client` and provides a `search_text` method that returns
/// parsed `Place` structs.
pub struct GooglePlacesClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl GooglePlacesClient {
    /// Create a new client with the given API key.
    pub fn new(api_key: String) -> Self {
        Self::with_base_url(api_key, DEFAULT_PLACES_BASE_URL.to_string())
    }

    /// Create a client targeting an explicit base URL.
    ///
    /// This is a non-behavioural seam used by tests to point the client at a
    /// mock server. Production callers should prefer [`GooglePlacesClient::new`],
    /// which uses the real Google Places New API base URL.
    pub(crate) fn with_base_url(api_key: String, base_url: String) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("valet/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            http,
            api_key,
            base_url,
        }
    }

    /// Search places by free-text query (e.g. "restaurantes en Lecce").
    pub async fn search_text(
        &self,
        query: &str,
        max_results: u32,
        location_bias: Option<(f64, f64, u32)>,
    ) -> Result<Vec<Place>, ToolError> {
        if self.api_key.is_empty() {
            return Err(ToolError::ExecutionError(
                "Google Places API key is not configured".into(),
            ));
        }

        let mut body = serde_json::json!({
            "textQuery": query,
            "maxResultCount": max_results,
            "languageCode": "es"
        });

        if let Some((lat, lon, radius)) = location_bias {
            body["locationBias"] = serde_json::json!({
                "circle": {
                    "center": {
                        "latitude": lat,
                        "longitude": lon
                    },
                    "radius": radius
                }
            });
        }

        let url = format!("{}/places:searchText", self.base_url);

        let resp = self
            .http
            .post(url)
            .header("X-Goog-Api-Key", &self.api_key)
            .header("X-Goog-FieldMask", FIELD_MASK)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                ToolError::ExecutionError(format!("Google Places API request failed: {}", e))
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionError(format!(
                "Google Places API returned {}: {}",
                status.as_u16(),
                body_text
            )));
        }

        let places_response: PlacesResponse = resp.json().await.map_err(|e| {
            ToolError::ExecutionError(format!("Failed to parse Google Places response: {}", e))
        })?;

        if let Some(ref error) = places_response.error {
            if let Some(ref msg) = error.message {
                return Err(ToolError::ExecutionError(format!(
                    "Google Places API error: {}",
                    msg
                )));
            }
        }

        Ok(places_response.places.unwrap_or_default())
    }

    /// Generate a Google Maps directions link from a list of places.
    ///
    /// Returns an empty string if no place has valid coordinates.
    pub fn maps_link(&self, places: &[Place]) -> String {
        let coords: Vec<String> = places
            .iter()
            .filter_map(|p| {
                p.location
                    .as_ref()
                    .and_then(|loc| match (loc.latitude, loc.longitude) {
                        (Some(lat), Some(lon)) => Some(format!("{:.5},{:.5}", lat, lon)),
                        _ => None,
                    })
            })
            .collect();

        if coords.is_empty() {
            return String::new();
        }

        format!("https://www.google.com/maps/dir/{}", coords.join("|"))
    }
}

// ---------------------------------------------------------------------------
// SearchPlacesTool — Tool trait implementation using Google Places
// ---------------------------------------------------------------------------

/// Tool that searches for places using the Google Places New API.
///
/// Reads the API key from the settings database at runtime
/// (key: `google_places_api_key`).
pub struct SearchPlacesTool {
    db: SqlitePool,
    base_url: String,
}

impl SearchPlacesTool {
    /// Create a new tool instance.
    ///
    /// The API key is **not** read at construction time; it is fetched from
    /// the settings database on each `execute` call.
    pub fn new(db: SqlitePool) -> Self {
        Self {
            db,
            base_url: DEFAULT_PLACES_BASE_URL.to_string(),
        }
    }

    /// Create a tool instance targeting an explicit base URL.
    ///
    /// Non-behavioural test seam: it lets tests redirect the underlying HTTP
    /// client to a mock server without altering production routing.
    #[cfg(test)]
    pub fn new_with_base_url(db: SqlitePool, base_url: String) -> Self {
        Self { db, base_url }
    }
}

#[async_trait]
impl Tool for SearchPlacesTool {
    fn name(&self) -> &'static str {
        "search_places"
    }

    fn description(&self) -> &'static str {
        "Busca lugares de interés, establecimientos o servicios usando Google Places API"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Tipo de lugar o nombre a buscar"
                },
                "latitude": {
                    "type": "number",
                    "description": "Latitud en grados decimales (opcional, para centrar la búsqueda)"
                },
                "longitude": {
                    "type": "number",
                    "description": "Longitud en grados decimales (opcional, para centrar la búsqueda)"
                },
                "radius": {
                    "type": "number",
                    "description": "Radio de búsqueda en metros (1-50000). Solo se aplica si se indican latitude y longitude"
                }
            },
            "required": ["query"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing query".into()))?;

        let lat = args.get("latitude").and_then(|v| v.as_f64());
        let lon = args.get("longitude").and_then(|v| v.as_f64());

        // Read API key from settings DB first, fallback to Config/ENV
        let api_key =
            match crate::db::repos::settings::SettingsRepo::get(&self.db, "google_places_api_key")
                .await
            {
                Ok(Some(key)) if !key.is_empty() => key,
                _ => crate::config::Config::from_env()
                    .google_places_api_key
                    .ok_or_else(|| {
                        ToolError::ExecutionError("Google Places API key is not configured".into())
                    })?,
            };

        let client = GooglePlacesClient::with_base_url(api_key, self.base_url.clone());

        // Always use `places:searchText` with the free-text query. A
        // `locationBias.circle` is applied only when both coordinates and a
        // valid numeric `radius` are supplied; otherwise the search runs with
        // no location bias at all.
        let location_bias = match (lat, lon, resolve_radius(args.get("radius"))) {
            (Some(lat), Some(lon), Some(radius)) => Some((lat, lon, radius)),
            _ => None,
        };

        let places = client.search_text(query, 10, location_bias).await?;

        let maps_link = client.maps_link(&places);

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "places": places,
                "maps_link": maps_link
            }),
            message: None,
        })
    }
}

/// Resolves the optional `radius` argument. A numeric value > 0 is rounded to
/// the nearest integer and clamped to Google's valid range [1, 50000]; anything
/// else (0, negative, null, non-numeric) yields no bias.
fn resolve_radius(value: Option<&serde_json::Value>) -> Option<u32> {
    let n = value?.as_f64()?;
    if n <= 0.0 {
        return None;
    }
    Some(n.round().clamp(1.0, 50000.0) as u32)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Deserialization tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_place_deserialization_full() {
        let json = serde_json::json!({
            "id": "ChIJN1t_tDeuEmsRUsoyG83frY4",
            "displayName": {
                "text": "La Parrilla",
                "languageCode": "es"
            },
            "formattedAddress": "Calle Mayor 10, 28013 Madrid, España",
            "location": {
                "latitude": 40.4168,
                "longitude": -3.7038
            },
            "rating": 4.5,
            "userRatingCount": 1234,
            "priceLevel": "PRICE_LEVEL_MODERATE",
            "websiteUri": "https://example.com",
            "nationalPhoneNumber": "+34 91 123 45 67",
            "regularOpeningHours": {
                "openNow": true,
                "weekdayDescriptions": [
                    "Monday: 9:00 AM – 10:00 PM",
                    "Tuesday: 9:00 AM – 10:00 PM"
                ]
            },
            "primaryType": "restaurant",
            "types": ["restaurant", "food"],
            "editorialSummary": {
                "text": "Un excelente restaurante en el centro de Madrid",
                "languageCode": "es"
            }
        });

        let place: Place = serde_json::from_value(json).unwrap();

        assert_eq!(place.id.as_deref(), Some("ChIJN1t_tDeuEmsRUsoyG83frY4"));
        assert_eq!(
            place.display_name.as_ref().and_then(|d| d.text.as_deref()),
            Some("La Parrilla")
        );
        assert_eq!(
            place.formatted_address.as_deref(),
            Some("Calle Mayor 10, 28013 Madrid, España")
        );
        assert_eq!(
            place.location.as_ref().and_then(|l| l.latitude),
            Some(40.4168)
        );
        assert_eq!(
            place.location.as_ref().and_then(|l| l.longitude),
            Some(-3.7038)
        );
        assert_eq!(place.rating, Some(4.5));
        assert_eq!(place.user_rating_count, Some(1234));
        assert_eq!(place.price_level.as_deref(), Some("PRICE_LEVEL_MODERATE"));
        assert_eq!(place.website_uri.as_deref(), Some("https://example.com"));
        assert_eq!(
            place
                .regular_opening_hours
                .as_ref()
                .and_then(|h| h.open_now),
            Some(true)
        );
        assert_eq!(place.primary_type.as_deref(), Some("restaurant"));
        assert_eq!(
            place.types.as_deref(),
            Some(&["restaurant".to_string(), "food".to_string()][..])
        );
        assert_eq!(
            place
                .editorial_summary
                .as_ref()
                .and_then(|s| s.text.as_deref()),
            Some("Un excelente restaurante en el centro de Madrid")
        );
    }

    #[test]
    fn test_place_deserialization_null_fields() {
        let json = serde_json::json!({
            "id": null,
            "displayName": null,
            "formattedAddress": null,
            "location": null,
            "rating": null,
            "userRatingCount": null,
            "priceLevel": null,
            "websiteUri": null,
            "nationalPhoneNumber": null,
            "regularOpeningHours": null,
            "primaryType": null,
            "types": null,
            "editorialSummary": null
        });

        let place: Place = serde_json::from_value(json).unwrap();

        assert!(place.id.is_none());
        assert!(place.display_name.is_none());
        assert!(place.formatted_address.is_none());
        assert!(place.location.is_none());
        assert!(place.rating.is_none());
        assert!(place.user_rating_count.is_none());
        assert!(place.price_level.is_none());
        assert!(place.website_uri.is_none());
        assert!(place.national_phone_number.is_none());
        assert!(place.regular_opening_hours.is_none());
        assert!(place.primary_type.is_none());
        assert!(place.types.is_none());
        assert!(place.editorial_summary.is_none());
    }

    #[test]
    fn test_places_response_deserialization() {
        let json = serde_json::json!({
            "places": [
                {
                    "id": "place-1",
                    "displayName": { "text": "Place One", "languageCode": "en" },
                    "location": { "latitude": 40.0, "longitude": -3.0 }
            },
                {
                    "id": "place-2",
                    "displayName": { "text": "Place Two", "languageCode": "en" },
                    "location": { "latitude": 41.0, "longitude": 2.0 }
                }
            ]
        });

        let response: PlacesResponse = serde_json::from_value(json).unwrap();

        assert!(response.error.is_none());
        let places = response.places.unwrap();
        assert_eq!(places.len(), 2);
        assert_eq!(places[0].id.as_deref(), Some("place-1"));
        assert_eq!(places[1].id.as_deref(), Some("place-2"));
    }

    // -----------------------------------------------------------------------
    // Maps link tests
    // -----------------------------------------------------------------------

    /// Helper: build a client with minimal fields for maps_link tests.
    fn test_client() -> GooglePlacesClient {
        GooglePlacesClient {
            http: reqwest::Client::new(),
            api_key: "test".into(),
            base_url: DEFAULT_PLACES_BASE_URL.into(),
        }
    }

    /// Helper: build a minimal Place with optional coordinates.
    fn place_with_coords(lat: Option<f64>, lon: Option<f64>) -> Place {
        Place {
            id: None,
            display_name: None,
            formatted_address: None,
            location: lat.zip(lon).map(|(lat, lon)| LatLng {
                latitude: Some(lat),
                longitude: Some(lon),
            }),
            rating: None,
            user_rating_count: None,
            price_level: None,
            website_uri: None,
            national_phone_number: None,
            regular_opening_hours: None,
            primary_type: None,
            types: None,
            editorial_summary: None,
        }
    }

    #[test]
    fn test_maps_link_empty() {
        let client = test_client();
        let link = client.maps_link(&[]);
        assert_eq!(link, "");
    }

    #[test]
    fn test_maps_link_single() {
        let client = test_client();
        let places = vec![place_with_coords(Some(40.3520), Some(18.1715))];
        let link = client.maps_link(&places);
        assert_eq!(link, "https://www.google.com/maps/dir/40.35200,18.17150");
    }

    #[test]
    fn test_maps_link_multiple() {
        let client = test_client();
        let places = vec![
            place_with_coords(Some(40.3520), Some(18.1715)),
            place_with_coords(Some(41.8930), Some(12.4820)),
        ];
        let link = client.maps_link(&places);
        assert_eq!(
            link,
            "https://www.google.com/maps/dir/40.35200,18.17150|41.89300,12.48200"
        );
    }

    #[test]
    fn test_maps_link_filters_without_coords() {
        let client = test_client();
        let places = vec![
            place_with_coords(None, None),
            place_with_coords(None, Some(12.4820)),
        ];
        let link = client.maps_link(&places);
        assert_eq!(link, "");
    }

    // -----------------------------------------------------------------------
    // Error handling — missing API key
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_search_text_missing_api_key() {
        let client = GooglePlacesClient::new(String::new());
        let result = client.search_text("cafe", 5, None).await;
        assert!(
            matches!(result, Err(ToolError::ExecutionError(ref msg)) if msg.contains("API key")),
            "Expected ExecutionError with 'API key' message, got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // Tool interface — SearchPlacesTool
    // -----------------------------------------------------------------------

    /// Helper: create a SearchPlacesTool with an in-memory DB (no API key set).
    async fn setup_tool() -> (SqlitePool, SearchPlacesTool) {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("Failed to create in-memory DB");

        crate::db::schema::run_migrations(&pool)
            .await
            .expect("Failed to run migrations");

        let tool = SearchPlacesTool::new(pool.clone());
        (pool, tool)
    }

    #[tokio::test]
    async fn test_search_places_name_and_description() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;
        assert_eq!(tool.name(), "search_places");
        assert!(
            tool.description().contains("Google"),
            "Description should mention Google: {}",
            tool.description()
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_search_places_permission() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;
        assert_eq!(
            tool.permission(&serde_json::json!({})),
            Permission::NoConfirm
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_search_places_parameters() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;
        let params = tool.parameters();
        assert_eq!(params["type"], "object");

        let required = params["required"].as_array().unwrap();
        let req_values: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(req_values, vec!["query"]);

        let props = params["properties"].as_object().unwrap();
        assert!(props.contains_key("query"), "Should have query");
        assert!(props.contains_key("latitude"), "Should have latitude");
        assert!(props.contains_key("longitude"), "Should have longitude");
        assert!(props.contains_key("radius"), "Should have radius");
        assert_eq!(props.len(), 4, "Should have four properties");

        let radius = &props["radius"];
        assert_eq!(
            radius["type"], "number",
            "radius must be a number so decimals are accepted, got: {radius}"
        );
        let radius_desc = radius["description"]
            .as_str()
            .expect("radius description should be a string");
        assert!(
            radius_desc.contains("1-50000"),
            "radius description must mention the 1-50000 range, got: {radius_desc}"
        );
        assert!(
            radius_desc.contains("latitude") && radius_desc.contains("longitude"),
            "radius description must mention it only applies with coordinates, got: {radius_desc}"
        );
        assert!(
            !radius_desc.contains("1000"),
            "radius description must NOT advertise a default of 1000, got: {radius_desc}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_search_places_missing_query() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;
        let result = tool
            .execute(serde_json::json!({
                "latitude": 40.41,
                "longitude": -3.70
            }))
            .await;
        assert!(
            matches!(result, Err(ToolError::InvalidArguments(_))),
            "Expected InvalidArguments error for missing query"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Endpoint usage — always searchText, optional locationBias (wiremock)
    // -----------------------------------------------------------------------

    /// Scenario: Búsqueda por texto (searchText)
    ///
    /// Without `radius`, `execute` must call `places:searchText` and the body
    /// must not carry `locationBias` nor `includedTypes`.
    #[tokio::test]
    async fn test_execute_without_radius_uses_search_text() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/places:searchText"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/places:searchNearby"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        let (pool, _) = setup_tool().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "google_places_api_key", "test-key")
            .await
            .expect("failed to seed google_places_api_key");
        let tool = SearchPlacesTool::new_with_base_url(pool, server.uri());

        let result = tool
            .execute(serde_json::json!({
                "query": "restaurantes en Madrid",
                "latitude": 40.4168,
                "longitude": -3.7038
            }))
            .await;
        assert!(result.is_ok(), "execute failed: {:?}", result.err());

        let requests = server
            .received_requests()
            .await
            .expect("received_requests should be available");
        let search_text_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchText")
            .collect();
        let paths: Vec<&str> = requests.iter().map(|r| r.url.path()).collect();

        assert_eq!(
            search_text_requests.len(),
            1,
            "expected exactly one POST /places:searchText, got request paths: {:?}",
            paths
        );

        let body: Value =
            serde_json::from_slice(&search_text_requests[0].body).expect("valid JSON body");
        assert_eq!(
            body["textQuery"], "restaurantes en Madrid",
            "textQuery must carry the query, got body: {body}"
        );
        assert_eq!(
            body["languageCode"], "es",
            "languageCode must be 'es', got body: {body}"
        );
        assert!(
            body.get("includedTypes").is_none(),
            "searchText body must NOT contain includedTypes, got body: {body}"
        );
    }

    /// Scenario: Nombre de negocio con radius
    ///
    /// Regression guard for the 400 `Unsupported types` bug: a business-name
    /// query with `radius` must go to `places:searchText` with a
    /// `locationBias.circle` and must NOT send `includedTypes`.
    #[tokio::test]
    async fn test_execute_name_with_radius_uses_search_text_bias() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/places:searchText"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/places:searchNearby"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        let (pool, _) = setup_tool().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "google_places_api_key", "test-key")
            .await
            .expect("failed to seed google_places_api_key");
        let tool = SearchPlacesTool::new_with_base_url(pool, server.uri());

        let result = tool
            .execute(serde_json::json!({
                "query": "Querida Jacinta restaurante",
                "latitude": 39.4676,
                "longitude": -0.3771,
                "radius": 5000
            }))
            .await;
        assert!(result.is_ok(), "execute failed: {:?}", result.err());

        let requests = server
            .received_requests()
            .await
            .expect("received_requests should be available");
        let paths: Vec<&str> = requests.iter().map(|r| r.url.path()).collect();
        let search_text_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchText")
            .collect();
        let nearby_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchNearby")
            .collect();

        assert_eq!(
            search_text_requests.len(),
            1,
            "expected exactly one POST /places:searchText, got request paths: {paths:?}"
        );
        assert_eq!(
            nearby_requests.len(),
            0,
            "must NOT call /places:searchNearby, got request paths: {paths:?}"
        );

        let body: Value =
            serde_json::from_slice(&search_text_requests[0].body).expect("valid JSON body");
        assert_eq!(
            body["textQuery"], "Querida Jacinta restaurante",
            "textQuery must carry the query, got body: {body}"
        );
        assert_eq!(
            body["locationBias"]["circle"]["center"]["latitude"],
            serde_json::json!(39.4676),
            "locationBias.circle.center.latitude must be 39.4676, got body: {body}"
        );
        assert_eq!(
            body["locationBias"]["circle"]["center"]["longitude"],
            serde_json::json!(-0.3771),
            "locationBias.circle.center.longitude must be -0.3771, got body: {body}"
        );
        assert_eq!(
            body["locationBias"]["circle"]["radius"],
            serde_json::json!(5000),
            "locationBias.circle.radius must be 5000, got body: {body}"
        );
        assert!(
            body.get("includedTypes").is_none(),
            "searchText body must NOT contain includedTypes, got body: {body}"
        );
    }

    /// Scenario: Búsqueda por texto (searchText) — sin radius no hay locationBias
    ///
    /// Guard test — without `radius` the body must not carry `locationBias`
    /// nor any `includedTypes`.
    #[tokio::test]
    async fn test_execute_without_radius_uses_search_text_without_bias() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/places:searchText"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/places:searchNearby"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        let (pool, _) = setup_tool().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "google_places_api_key", "test-key")
            .await
            .expect("failed to seed google_places_api_key");
        let tool = SearchPlacesTool::new_with_base_url(pool, server.uri());

        let result = tool
            .execute(serde_json::json!({
                "query": "restaurantes en Madrid",
                "latitude": 40.4168,
                "longitude": -3.7038
            }))
            .await;
        assert!(result.is_ok(), "execute failed: {:?}", result.err());

        let requests = server
            .received_requests()
            .await
            .expect("received_requests should be available");
        let paths: Vec<&str> = requests.iter().map(|r| r.url.path()).collect();
        let search_text_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchText")
            .collect();
        let nearby_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchNearby")
            .collect();

        assert_eq!(
            search_text_requests.len(),
            1,
            "expected exactly one POST /places:searchText, got request paths: {paths:?}"
        );
        assert_eq!(
            nearby_requests.len(),
            0,
            "must NOT call /places:searchNearby, got request paths: {paths:?}"
        );

        let body: Value =
            serde_json::from_slice(&search_text_requests[0].body).expect("valid JSON body");
        assert_eq!(
            body["textQuery"], "restaurantes en Madrid",
            "textQuery must carry the query, got body: {body}"
        );
        assert!(
            body.get("locationBias").is_none(),
            "body must NOT contain locationBias when radius is absent, got body: {body}"
        );
        assert!(
            body.get("includedTypes").is_none(),
            "searchText body must NOT contain includedTypes, got body: {body}"
        );
    }

    /// Scenario: Búsqueda por cercanía (searchNearby) — tipo con radius
    ///
    /// A type query with `radius` must also route to `places:searchText` with a
    /// `locationBias.circle` (radius 500) and no `includedTypes`.
    #[tokio::test]
    async fn test_execute_type_with_radius_uses_search_text_bias() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/places:searchText"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/places:searchNearby"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        let (pool, _) = setup_tool().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "google_places_api_key", "test-key")
            .await
            .expect("failed to seed google_places_api_key");
        let tool = SearchPlacesTool::new_with_base_url(pool, server.uri());

        let result = tool
            .execute(serde_json::json!({
                "query": "cafe",
                "latitude": 40.4168,
                "longitude": -3.7038,
                "radius": 500
            }))
            .await;
        assert!(result.is_ok(), "execute failed: {:?}", result.err());

        let requests = server
            .received_requests()
            .await
            .expect("received_requests should be available");
        let paths: Vec<&str> = requests.iter().map(|r| r.url.path()).collect();
        let search_text_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchText")
            .collect();
        let nearby_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchNearby")
            .collect();

        assert_eq!(
            search_text_requests.len(),
            1,
            "expected exactly one POST /places:searchText, got request paths: {paths:?}"
        );
        assert_eq!(
            nearby_requests.len(),
            0,
            "must NOT call /places:searchNearby, got request paths: {paths:?}"
        );

        let body: Value =
            serde_json::from_slice(&search_text_requests[0].body).expect("valid JSON body");
        assert_eq!(
            body["textQuery"], "cafe",
            "textQuery must carry the query, got body: {body}"
        );
        assert_eq!(
            body["locationBias"]["circle"]["center"]["latitude"],
            serde_json::json!(40.4168),
            "locationBias.circle.center.latitude must be 40.4168, got body: {body}"
        );
        assert_eq!(
            body["locationBias"]["circle"]["center"]["longitude"],
            serde_json::json!(-3.7038),
            "locationBias.circle.center.longitude must be -3.7038, got body: {body}"
        );
        assert_eq!(
            body["locationBias"]["circle"]["radius"],
            serde_json::json!(500),
            "locationBias.circle.radius must be 500, got body: {body}"
        );
        assert!(
            body.get("includedTypes").is_none(),
            "searchText body must NOT contain includedTypes, got body: {body}"
        );
    }

    /// Scenario: Radio por encima del máximo
    ///
    /// A `radius` greater than 50000 must be clamped to 50000 before being
    /// sent in `locationBias.circle.radius`.
    #[tokio::test]
    async fn test_execute_radius_above_max_is_clamped() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/places:searchText"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/places:searchNearby"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        let (pool, _) = setup_tool().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "google_places_api_key", "test-key")
            .await
            .expect("failed to seed google_places_api_key");
        let tool = SearchPlacesTool::new_with_base_url(pool, server.uri());

        let result = tool
            .execute(serde_json::json!({
                "query": "cafe",
                "latitude": 40.4168,
                "longitude": -3.7038,
                "radius": 100000
            }))
            .await;
        assert!(result.is_ok(), "execute failed: {:?}", result.err());

        let requests = server
            .received_requests()
            .await
            .expect("received_requests should be available");
        let paths: Vec<&str> = requests.iter().map(|r| r.url.path()).collect();
        let search_text_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchText")
            .collect();
        let nearby_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchNearby")
            .collect();

        assert_eq!(
            search_text_requests.len(),
            1,
            "expected exactly one POST /places:searchText, got request paths: {paths:?}"
        );
        assert_eq!(
            nearby_requests.len(),
            0,
            "must NOT call /places:searchNearby, got request paths: {paths:?}"
        );

        let body: Value =
            serde_json::from_slice(&search_text_requests[0].body).expect("valid JSON body");
        assert_eq!(
            body["textQuery"], "cafe",
            "textQuery must carry the query, got body: {body}"
        );
        assert_eq!(
            body["locationBias"]["circle"]["radius"],
            serde_json::json!(50000),
            "locationBias.circle.radius must be clamped to 50000, got body: {body}"
        );
    }

    /// Scenario: Radio decimal
    ///
    /// A decimal `radius` must be rounded to the nearest integer (2500.6 ->
    /// 2501) and sent in `locationBias.circle.radius`.
    #[tokio::test]
    async fn test_execute_radius_decimal_is_rounded() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/places:searchText"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/places:searchNearby"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        let (pool, _) = setup_tool().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "google_places_api_key", "test-key")
            .await
            .expect("failed to seed google_places_api_key");
        let tool = SearchPlacesTool::new_with_base_url(pool, server.uri());

        let result = tool
            .execute(serde_json::json!({
                "query": "cafe",
                "latitude": 40.4168,
                "longitude": -3.7038,
                "radius": 2500.6
            }))
            .await;
        assert!(result.is_ok(), "execute failed: {:?}", result.err());

        let requests = server
            .received_requests()
            .await
            .expect("received_requests should be available");
        let paths: Vec<&str> = requests.iter().map(|r| r.url.path()).collect();
        let search_text_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchText")
            .collect();
        let nearby_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchNearby")
            .collect();

        assert_eq!(
            search_text_requests.len(),
            1,
            "expected exactly one POST /places:searchText, got request paths: {paths:?}"
        );
        assert_eq!(
            nearby_requests.len(),
            0,
            "must NOT call /places:searchNearby, got request paths: {paths:?}"
        );

        let body: Value =
            serde_json::from_slice(&search_text_requests[0].body).expect("valid JSON body");
        assert_eq!(
            body["textQuery"], "cafe",
            "textQuery must carry the query, got body: {body}"
        );
        assert_eq!(
            body["locationBias"]["circle"]["radius"],
            serde_json::json!(2501),
            "locationBias.circle.radius must be rounded to 2501, got body: {body}"
        );
    }

    /// Scenario: Radio no numérico
    ///
    /// A non-numeric `radius` must be ignored: the request must still go to
    /// `places:searchText` but must NOT include `locationBias` (nor
    /// `includedTypes`).
    #[tokio::test]
    async fn test_execute_radius_non_numeric_has_no_location_bias() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/places:searchText"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/places:searchNearby"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        let (pool, _) = setup_tool().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "google_places_api_key", "test-key")
            .await
            .expect("failed to seed google_places_api_key");
        let tool = SearchPlacesTool::new_with_base_url(pool, server.uri());

        let result = tool
            .execute(serde_json::json!({
                "query": "cafe",
                "latitude": 40.4168,
                "longitude": -3.7038,
                "radius": "cerca"
            }))
            .await;
        assert!(result.is_ok(), "execute failed: {:?}", result.err());

        let requests = server
            .received_requests()
            .await
            .expect("received_requests should be available");
        let paths: Vec<&str> = requests.iter().map(|r| r.url.path()).collect();
        let search_text_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchText")
            .collect();
        let nearby_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchNearby")
            .collect();

        assert_eq!(
            search_text_requests.len(),
            1,
            "expected exactly one POST /places:searchText, got request paths: {paths:?}"
        );
        assert_eq!(
            nearby_requests.len(),
            0,
            "must NOT call /places:searchNearby, got request paths: {paths:?}"
        );

        let body: Value =
            serde_json::from_slice(&search_text_requests[0].body).expect("valid JSON body");
        assert_eq!(
            body["textQuery"], "cafe",
            "textQuery must carry the query, got body: {body}"
        );
        assert!(
            body.get("locationBias").is_none(),
            "body must NOT contain locationBias for a non-numeric radius, got body: {body}"
        );
        assert!(
            body.get("includedTypes").is_none(),
            "searchText body must NOT contain includedTypes, got body: {body}"
        );
    }

    /// Scenario: Radio cero o negativo
    ///
    /// A `radius` of 0 must be ignored: the request must go to
    /// `places:searchText` without `locationBias`.
    #[tokio::test]
    async fn test_execute_radius_zero_has_no_location_bias() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/places:searchText"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/places:searchNearby"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        let (pool, _) = setup_tool().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "google_places_api_key", "test-key")
            .await
            .expect("failed to seed google_places_api_key");
        let tool = SearchPlacesTool::new_with_base_url(pool, server.uri());

        let result = tool
            .execute(serde_json::json!({
                "query": "cafe",
                "latitude": 40.4168,
                "longitude": -3.7038,
                "radius": 0
            }))
            .await;
        assert!(result.is_ok(), "execute failed: {:?}", result.err());

        let requests = server
            .received_requests()
            .await
            .expect("received_requests should be available");
        let paths: Vec<&str> = requests.iter().map(|r| r.url.path()).collect();
        let search_text_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchText")
            .collect();
        let nearby_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchNearby")
            .collect();

        assert_eq!(
            search_text_requests.len(),
            1,
            "expected exactly one POST /places:searchText, got request paths: {paths:?}"
        );
        assert_eq!(
            nearby_requests.len(),
            0,
            "must NOT call /places:searchNearby, got request paths: {paths:?}"
        );

        let body: Value =
            serde_json::from_slice(&search_text_requests[0].body).expect("valid JSON body");
        assert_eq!(
            body["textQuery"], "cafe",
            "textQuery must carry the query, got body: {body}"
        );
        assert!(
            body.get("locationBias").is_none(),
            "body must NOT contain locationBias for radius 0, got body: {body}"
        );
    }

    // -----------------------------------------------------------------------
    // resolve_radius unit tests (pure function boundaries)
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_radius_valid_boundaries() {
        assert_eq!(resolve_radius(Some(&serde_json::json!(0.4))), Some(1));
        assert_eq!(resolve_radius(Some(&serde_json::json!(1))), Some(1));
        assert_eq!(resolve_radius(Some(&serde_json::json!(2500.6))), Some(2501));
        assert_eq!(resolve_radius(Some(&serde_json::json!(50000))), Some(50000));
        assert_eq!(
            resolve_radius(Some(&serde_json::json!(50000.6))),
            Some(50000)
        );
        assert_eq!(
            resolve_radius(Some(&serde_json::json!(100000))),
            Some(50000)
        );
    }

    #[test]
    fn test_resolve_radius_invalid_yields_none() {
        assert_eq!(resolve_radius(Some(&serde_json::json!(0))), None);
        assert_eq!(resolve_radius(Some(&serde_json::json!(-100))), None);
        assert_eq!(resolve_radius(Some(&serde_json::json!(null))), None);
        assert_eq!(resolve_radius(Some(&serde_json::json!("cerca"))), None);
    }

    // -----------------------------------------------------------------------
    // Endpoint usage — invalid radius never applies locationBias (wiremock)
    // -----------------------------------------------------------------------

    /// Runs `execute` against a wiremock server with the given `radius` value
    /// and returns the JSON body posted to `/places:searchText`, asserting the
    /// request went there (and not to `/places:searchNearby`).
    async fn execute_and_capture_search_text_body(radius: Value) -> Value {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/places:searchText"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/places:searchNearby"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        let (pool, _) = setup_tool().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "google_places_api_key", "test-key")
            .await
            .expect("failed to seed google_places_api_key");
        let tool = SearchPlacesTool::new_with_base_url(pool, server.uri());

        let result = tool
            .execute(serde_json::json!({
                "query": "cafe",
                "latitude": 40.4168,
                "longitude": -3.7038,
                "radius": radius
            }))
            .await;
        assert!(result.is_ok(), "execute failed: {:?}", result.err());

        let requests = server
            .received_requests()
            .await
            .expect("received_requests should be available");
        let paths: Vec<&str> = requests.iter().map(|r| r.url.path()).collect();
        let search_text_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchText")
            .collect();
        let nearby_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchNearby")
            .collect();

        assert_eq!(
            search_text_requests.len(),
            1,
            "expected exactly one POST /places:searchText, got request paths: {paths:?}"
        );
        assert_eq!(
            nearby_requests.len(),
            0,
            "must NOT call /places:searchNearby, got request paths: {paths:?}"
        );

        serde_json::from_slice(&search_text_requests[0].body).expect("valid JSON body")
    }

    #[tokio::test]
    async fn test_execute_radius_negative_has_no_location_bias() {
        let body = execute_and_capture_search_text_body(serde_json::json!(-100)).await;
        assert_eq!(
            body["textQuery"], "cafe",
            "textQuery must carry the query, got body: {body}"
        );
        assert!(
            body.get("locationBias").is_none(),
            "body must NOT contain locationBias for a negative radius, got body: {body}"
        );
    }

    #[tokio::test]
    async fn test_execute_radius_null_has_no_location_bias() {
        let body = execute_and_capture_search_text_body(serde_json::json!(null)).await;
        assert_eq!(
            body["textQuery"], "cafe",
            "textQuery must carry the query, got body: {body}"
        );
        assert!(
            body.get("locationBias").is_none(),
            "body must NOT contain locationBias for a null radius, got body: {body}"
        );
    }

    // -----------------------------------------------------------------------
    // RED — improve-tool-schemas: search_places requires only `query` and its
    // definitions must be translated to Spanish.
    // -----------------------------------------------------------------------

    /// Scenario: search_places tool requires only query
    #[tokio::test]
    async fn test_search_places_parameters_requires_only_query(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;
        let params = tool.parameters();
        assert_eq!(
            params["required"],
            serde_json::json!(["query"]),
            "search_places must require only `query`, got: {}",
            params["required"]
        );
        Ok(())
    }

    /// Scenario: Las tools en inglés se traducen al español
    ///
    /// `description()` and the descriptions of `query`, `latitude`, `longitude`
    /// and `radius` must not contain the known English phrases.
    #[tokio::test]
    async fn test_search_places_descriptions_are_in_spanish(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;
        let params = tool.parameters();
        let props = params["properties"].as_object().unwrap();

        // Known English phrases that must NOT appear once the definitions are
        // translated to Spanish.
        let forbidden = [
            "search for places",
            "place name or type",
            "latitude in",
            "longitude in",
            "search radius",
            "using google places api",
        ];

        let tool_desc = tool.description();
        let tool_desc_lower = tool_desc.to_lowercase();
        for phrase in forbidden {
            assert!(
                !tool_desc_lower.contains(phrase),
                "description() must be in Spanish, found English phrase `{phrase}`: {tool_desc}"
            );
        }

        let param_descs = [
            (
                "query",
                props["query"]["description"].as_str().unwrap_or(""),
            ),
            (
                "latitude",
                props["latitude"]["description"].as_str().unwrap_or(""),
            ),
            (
                "longitude",
                props["longitude"]["description"].as_str().unwrap_or(""),
            ),
            (
                "radius",
                props["radius"]["description"].as_str().unwrap_or(""),
            ),
        ];
        for (name, desc) in param_descs {
            let lower = desc.to_lowercase();
            for phrase in forbidden {
                assert!(
                    !lower.contains(phrase),
                    "parameter `{name}` description must be in Spanish, found `{phrase}`: {desc}"
                );
            }
        }
        Ok(())
    }

    /// Scenario: Búsqueda por texto sin coordenadas
    ///
    /// With only `query`, `execute` must call `places:searchText`, carry
    /// `textQuery` and `languageCode: "es"`, and must NOT include
    /// `locationBias` nor `includedTypes`.
    #[tokio::test]
    async fn test_execute_without_coordinates_uses_search_text_without_bias() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/places:searchText"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/places:searchNearby"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        let (pool, _) = setup_tool().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "google_places_api_key", "test-key")
            .await
            .expect("failed to seed google_places_api_key");
        let tool = SearchPlacesTool::new_with_base_url(pool, server.uri());

        let result = tool
            .execute(serde_json::json!({ "query": "museos en Madrid" }))
            .await;
        assert!(result.is_ok(), "execute failed: {:?}", result.err());

        let requests = server
            .received_requests()
            .await
            .expect("received_requests should be available");
        let paths: Vec<&str> = requests.iter().map(|r| r.url.path()).collect();
        let search_text_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchText")
            .collect();

        assert_eq!(
            search_text_requests.len(),
            1,
            "expected exactly one POST /places:searchText, got request paths: {paths:?}"
        );

        let body: Value =
            serde_json::from_slice(&search_text_requests[0].body).expect("valid JSON body");
        assert_eq!(
            body["textQuery"], "museos en Madrid",
            "textQuery must carry the query, got body: {body}"
        );
        assert_eq!(
            body["languageCode"], "es",
            "languageCode must be 'es', got body: {body}"
        );
        assert!(
            body.get("locationBias").is_none(),
            "body must NOT contain locationBias without coordinates, got body: {body}"
        );
        assert!(
            body.get("includedTypes").is_none(),
            "body must NOT contain includedTypes, got body: {body}"
        );
    }

    /// Scenario: Radio sin coordenadas no aplica sesgo
    ///
    /// With `query` + `radius` but no coordinates, `execute` must succeed,
    /// route to `places:searchText` and must NOT include `locationBias`.
    #[tokio::test]
    async fn test_execute_radius_without_coordinates_has_no_bias() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/places:searchText"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/places:searchNearby"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
            )
            .mount(&server)
            .await;

        let (pool, _) = setup_tool().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "google_places_api_key", "test-key")
            .await
            .expect("failed to seed google_places_api_key");
        let tool = SearchPlacesTool::new_with_base_url(pool, server.uri());

        let result = tool
            .execute(serde_json::json!({ "query": "cafe", "radius": 500 }))
            .await;
        assert!(result.is_ok(), "execute failed: {:?}", result.err());

        let requests = server
            .received_requests()
            .await
            .expect("received_requests should be available");
        let paths: Vec<&str> = requests.iter().map(|r| r.url.path()).collect();
        let search_text_requests: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/places:searchText")
            .collect();

        assert_eq!(
            search_text_requests.len(),
            1,
            "expected exactly one POST /places:searchText, got request paths: {paths:?}"
        );

        let body: Value =
            serde_json::from_slice(&search_text_requests[0].body).expect("valid JSON body");
        assert_eq!(
            body["textQuery"], "cafe",
            "textQuery must carry the query, got body: {body}"
        );
        assert!(
            body.get("locationBias").is_none(),
            "body must NOT contain locationBias without coordinates, got body: {body}"
        );
    }

    /// Scenario: Coordenadas parciales no aplican sesgo
    ///
    /// Regression guard for the removed `test_search_places_missing_lat`: when
    /// only one of `latitude`/`longitude` is supplied, `execute` must still
    /// succeed, route to `places:searchText`, and must NOT include
    /// `locationBias` (nor `includedTypes`).
    #[tokio::test]
    async fn test_execute_partial_coordinates_have_no_bias() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let cases = [
            serde_json::json!({ "query": "cafe", "longitude": -3.70 }),
            serde_json::json!({ "query": "cafe", "latitude": 40.4168 }),
        ];

        for args in cases {
            let server = MockServer::start().await;

            Mock::given(method("POST"))
                .and(path("/places:searchText"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
                )
                .mount(&server)
                .await;

            Mock::given(method("POST"))
                .and(path("/places:searchNearby"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({ "places": [] })),
                )
                .mount(&server)
                .await;

            let (pool, _) = setup_tool().await;
            crate::db::repos::settings::SettingsRepo::set(
                &pool,
                "google_places_api_key",
                "test-key",
            )
            .await
            .expect("failed to seed google_places_api_key");
            let tool = SearchPlacesTool::new_with_base_url(pool, server.uri());

            let result = tool.execute(args.clone()).await;
            assert!(
                result.is_ok(),
                "execute must succeed for partial coordinates {args}, got: {:?}",
                result.err()
            );

            let requests = server
                .received_requests()
                .await
                .expect("received_requests should be available");
            let paths: Vec<&str> = requests.iter().map(|r| r.url.path()).collect();
            let search_text_requests: Vec<_> = requests
                .iter()
                .filter(|r| r.url.path() == "/places:searchText")
                .collect();

            assert_eq!(
                search_text_requests.len(),
                1,
                "expected exactly one POST /places:searchText for {args}, got request paths: {paths:?}"
            );

            let body: Value =
                serde_json::from_slice(&search_text_requests[0].body).expect("valid JSON body");
            assert!(
                body.get("locationBias").is_none(),
                "body must NOT contain locationBias for partial coordinates {args}, got body: {body}"
            );
            assert!(
                body.get("includedTypes").is_none(),
                "searchText body must NOT contain includedTypes for {args}, got body: {body}"
            );
        }
    }
}
