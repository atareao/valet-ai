use async_trait::async_trait;
use serde_json::Value;

use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

// ===========================================================================
// GeocodeTool — geocodificar una dirección a coordenadas
// ===========================================================================

pub struct GeocodeTool {
    client: reqwest::Client,
}

impl GeocodeTool {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Valet/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build reqwest Client");
        Self { client }
    }

    /// Convert a Nominatim result JSON object into the standard geo result format.
    pub(crate) fn nominatim_to_geo_result(item: &Value) -> Value {
        let address = item.get("address").and_then(|a| a.as_object()).map(|addr| {
            serde_json::json!({
                "road": addr.get("road").or_else(|| addr.get("pedestrian")).or_else(|| addr.get("footway")).and_then(|v| v.as_str()).unwrap_or(""),
                "city": addr.get("city").or_else(|| addr.get("town")).or_else(|| addr.get("village")).or_else(|| addr.get("municipality")).and_then(|v| v.as_str()).unwrap_or(""),
                "state": addr.get("state").and_then(|v| v.as_str()).unwrap_or(""),
                "country": addr.get("country").and_then(|v| v.as_str()).unwrap_or(""),
                "postcode": addr.get("postcode").and_then(|v| v.as_str()).unwrap_or(""),
            })
        }).unwrap_or_default();

        serde_json::json!({
            "latitude": item["lat"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0),
            "longitude": item["lon"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0),
            "display_name": item["display_name"].as_str().unwrap_or(""),
            "address": address,
        })
    }
}

#[async_trait]
impl Tool for GeocodeTool {
    fn name(&self) -> &'static str {
        "geocode"
    }

    fn description(&self) -> &'static str {
        "Geocodifica una dirección o un lugar conocido (calle, ciudad o monumento) y devuelve sus coordenadas geográficas junto con la dirección desglosada."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Dirección o lugar a geocodificar"
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

        let url = format!(
            "https://nominatim.openstreetmap.org/search?q={}&format=json&limit=1&addressdetails=1",
            urlencoding(query)
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Nominatim request failed: {e}")))?;

        let body: Value = resp
            .json()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Nominatim parse failed: {e}")))?;

        let results = body
            .as_array()
            .ok_or_else(|| ToolError::ExecutionError("Unexpected Nominatim response".into()))?;

        let first = results
            .first()
            .ok_or_else(|| ToolError::NotFound("No results found".into()))?;

        Ok(ToolResult {
            success: true,
            data: Self::nominatim_to_geo_result(first),
            message: None,
        })
    }
}

impl Default for GeocodeTool {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// ReverseGeocodeTool — obtener dirección a partir de coordenadas
// ===========================================================================

pub struct ReverseGeocodeTool {
    client: reqwest::Client,
}

impl ReverseGeocodeTool {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Valet/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build reqwest Client");
        Self { client }
    }
}

#[async_trait]
impl Tool for ReverseGeocodeTool {
    fn name(&self) -> &'static str {
        "reverse_geocode"
    }

    fn description(&self) -> &'static str {
        "Obtiene la dirección y los detalles del lugar correspondientes a unas coordenadas geográficas de latitud y longitud."
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
                }
            },
            "required": ["latitude", "longitude"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let lat = args
            .get("latitude")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ToolError::InvalidArguments("Missing or invalid latitude".into()))?;
        let lon = args
            .get("longitude")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ToolError::InvalidArguments("Missing or invalid longitude".into()))?;

        let url = format!(
            "https://nominatim.openstreetmap.org/reverse?lat={}&lon={}&format=json&addressdetails=1",
            lat, lon
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Nominatim request failed: {e}")))?;

        let body: Value = resp
            .json()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Nominatim parse failed: {e}")))?;

        if body.get("error").is_some() {
            return Err(ToolError::NotFound(
                body["error"]
                    .as_str()
                    .unwrap_or("No results found")
                    .to_string(),
            ));
        }

        Ok(ToolResult {
            success: true,
            data: GeocodeTool::nominatim_to_geo_result(&body),
            message: None,
        })
    }
}

impl Default for ReverseGeocodeTool {
    fn default() -> Self {
        Self::new()
    }
}

/// URL-encode a string for use in a query parameter.
fn urlencoding(s: &str) -> String {
    let mut encoded = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // GeocodeTool tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_geocode_name_and_description() -> Result<(), Box<dyn std::error::Error>> {
        let tool = GeocodeTool::new();
        assert_eq!(tool.name(), "geocode");
        assert_eq!(
            tool.description(),
            "Geocodifica una dirección o un lugar conocido (calle, ciudad o monumento) y devuelve sus coordenadas geográficas junto con la dirección desglosada."
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_geocode_permission() -> Result<(), Box<dyn std::error::Error>> {
        let tool = GeocodeTool::new();
        assert_eq!(
            tool.permission(&serde_json::json!({})),
            Permission::NoConfirm
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_geocode_parameters_has_only_query() -> Result<(), Box<dyn std::error::Error>> {
        let tool = GeocodeTool::new();
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        let required = params["required"].as_array().unwrap();
        let req_values: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(req_values, vec!["query"]);
        let props = params["properties"].as_object().unwrap();
        assert!(props.contains_key("query"), "Should have query");
        assert_eq!(props["query"]["type"], "string", "query should be a string");
        assert_eq!(props.len(), 1, "Should only have one property: query");
        Ok(())
    }

    #[tokio::test]
    async fn test_geocode_missing_query_returns_error() -> Result<(), Box<dyn std::error::Error>> {
        let tool = GeocodeTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(
            matches!(result, Err(ToolError::InvalidArguments(_))),
            "Expected InvalidArguments error for missing query"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // ReverseGeocodeTool tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_reverse_geocode_name_and_description() -> Result<(), Box<dyn std::error::Error>> {
        let tool = ReverseGeocodeTool::new();
        assert_eq!(tool.name(), "reverse_geocode");
        assert_eq!(
            tool.description(),
            "Obtiene la dirección y los detalles del lugar correspondientes a unas coordenadas geográficas de latitud y longitud."
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_reverse_geocode_permission() -> Result<(), Box<dyn std::error::Error>> {
        let tool = ReverseGeocodeTool::new();
        assert_eq!(
            tool.permission(&serde_json::json!({})),
            Permission::NoConfirm
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_reverse_geocode_parameters_has_lat_lon() -> Result<(), Box<dyn std::error::Error>>
    {
        let tool = ReverseGeocodeTool::new();
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        let required = params["required"].as_array().unwrap();
        let req_values: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(req_values, vec!["latitude", "longitude"]);
        let props = params["properties"].as_object().unwrap();
        assert!(props.contains_key("latitude"), "Should have latitude");
        assert!(props.contains_key("longitude"), "Should have longitude");
        assert_eq!(props.len(), 2, "Should only have two properties");
        Ok(())
    }

    #[tokio::test]
    async fn test_reverse_geocode_missing_lat_returns_error(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tool = ReverseGeocodeTool::new();
        let result = tool.execute(serde_json::json!({"longitude": -3.70})).await;
        assert!(
            matches!(result, Err(ToolError::InvalidArguments(_))),
            "Expected InvalidArguments error for missing latitude"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_reverse_geocode_missing_lon_returns_error(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tool = ReverseGeocodeTool::new();
        let result = tool.execute(serde_json::json!({"latitude": 40.41})).await;
        assert!(
            matches!(result, Err(ToolError::InvalidArguments(_))),
            "Expected InvalidArguments error for missing longitude"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Unit tests for helper functions
    // -----------------------------------------------------------------------

    #[test]
    fn test_urlencoding_encodes_spaces_and_specials() {
        assert_eq!(urlencoding("hello world"), "hello%20world");
        assert_eq!(urlencoding("a/b"), "a%2Fb");
        assert_eq!(urlencoding("alphanumeric123"), "alphanumeric123");
    }

    #[test]
    fn test_nominatim_to_geo_result_parses_string_coords() {
        let item = serde_json::json!({
            "lat": "40.4155",
            "lon": "-3.7074",
            "display_name": "Plaza Mayor, Madrid, España",
            "address": {
                "road": "Plaza Mayor",
                "city": "Madrid",
                "state": "Comunidad de Madrid",
                "country": "España",
                "postcode": "28012"
            }
        });
        let result = GeocodeTool::nominatim_to_geo_result(&item);
        assert_eq!(result["latitude"].as_f64(), Some(40.4155));
        assert_eq!(result["longitude"].as_f64(), Some(-3.7074));
        assert_eq!(result["display_name"], "Plaza Mayor, Madrid, España");
        assert_eq!(result["address"]["city"], "Madrid");
        assert_eq!(result["address"]["postcode"], "28012");
    }

    // -----------------------------------------------------------------------
    // Language guard — geocode / reverse_geocode descriptions in Spanish
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_geocode_and_reverse_geocode_descriptions_are_spanish() {
        let geocode = GeocodeTool::new();
        let g = geocode.description();
        assert!(
            g.contains("dirección"),
            "geocode description must be in Spanish (must contain 'dirección'), got: {g}"
        );
        assert!(
            g.contains("coordenadas"),
            "geocode description must be in Spanish (must contain 'coordenadas'), got: {g}"
        );

        let reverse = ReverseGeocodeTool::new();
        let r = reverse.description();
        assert!(
            r.contains("dirección"),
            "reverse_geocode description must be in Spanish, got: {r}"
        );
        assert!(
            r.contains("coordenadas"),
            "reverse_geocode description must be in Spanish, got: {r}"
        );
    }
}
