//! Contrato de la Decisions API de Jev y su cliente HTTP.
//!
//! El módulo define el trait y los tipos que el router necesita para compilar
//! y para inyectar un doble determinista en los tests, y además el cliente
//! real (`JevDecisionsProvider`) que habla el endpoint `alpha` con `reqwest`.
//!
//! El cliente se construye desde el entorno con [`JevDecisionsConfig::from_env`]:
//! si falta `OPENROUTER_API_KEY` o está vacía devuelve `None`, y es así como el
//! orquestador decide que no hay clasificador y el enrutado cae a fallo abierto.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

use crate::llm::provider::LLMError;

/// Application name sent to the Decisions API for identification.
const APP_NAME: &str = "Valet";
/// Application URL sent to the Decisions API for identification.
const APP_URL: &str = "https://github.com/atareao/valet-ai";
/// Default base URL for the Decisions API when `ROUTER_BASE_URL` is unset.
const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api";

/// Una pregunta tipada para el modelo de decisiones.
///
/// En esta entrega todas las preguntas son de tipo `noul` («¿hace falta esta
/// habilidad?»): llevan las instrucciones y las `criteria` del sí y del no.
#[derive(Debug, Clone)]
pub struct DecisionsQuestion {
    /// Identificador de la pregunta, estable y trazable (p. ej. el id de skill).
    pub id: String,
    /// Qué se le pregunta al modelo.
    pub instructions: String,
    /// Descripción de cuándo la respuesta es «sí».
    pub criteria_true: String,
    /// Descripción de cuándo la respuesta es «no».
    pub criteria_false: String,
}

/// Petición a la Decisions API: estado textual + preguntas tipadas.
#[derive(Debug, Clone)]
pub struct DecisionsRequest {
    /// Modelo de decisiones a usar (p. ej. `typesafe/jev-1.13`).
    pub model: String,
    /// Estado textual (mensaje actual + últimos turnos).
    pub state: serde_json::Value,
    /// Preguntas a responder en paralelo dentro de la misma petición.
    pub questions: Vec<DecisionsQuestion>,
}

/// Respuesta tipada: probabilidad de sí por id de pregunta + uso.
#[derive(Debug, Clone, Default)]
pub struct DecisionsResponse {
    /// Probabilidad de «sí» por id de pregunta.
    pub answers: HashMap<String, f32>,
    /// Tokens de entrada consumidos.
    pub input_tokens: u64,
    /// Tokens de salida generados.
    pub output_tokens: u64,
    /// Coste devuelto por el servicio.
    pub cost: f64,
}

/// Trait inyectable del clasificador de decisiones.
///
/// El router depende de esta abstracción, no del cliente concreto, de modo
/// que las pruebas puedan inyectar un doble determinista sin red.
#[async_trait]
pub trait DecisionsProvider: Send + Sync {
    /// Responde a la petición de decisiones.
    async fn decide(&self, request: DecisionsRequest) -> Result<DecisionsResponse, LLMError>;
}

/// Configuración del cliente real de la Decisions API de Jev.
///
/// La credencial y el endpoint se toman del entorno (`OPENROUTER_API_KEY` y
/// `ROUTER_BASE_URL`); el modelo y el timeout se inyectan desde los ajustes.
#[derive(Debug, Clone)]
pub struct JevDecisionsConfig {
    /// Credencial de OpenRouter enviada en la cabecera de autorización.
    pub api_key: String,
    /// Base del endpoint (sin la parte `/alpha/decisions`).
    pub base_url: String,
    /// Modelo de decisiones (p. ej. `typesafe/jev-1.13`).
    pub model: String,
    /// Timeout propio del cliente, en milisegundos; independiente del chat.
    pub timeout_ms: u64,
}

impl JevDecisionsConfig {
    /// Construye la configuración desde el entorno.
    ///
    /// Lee `OPENROUTER_API_KEY` (la misma del chat) y `ROUTER_BASE_URL`, que
    /// por defecto es [`DEFAULT_BASE_URL`]. Devuelve `None` cuando la API key
    /// falta **o está vacía**, que es como el orquestador detecta que **no** hay
    /// clasificador y el enrutado cae a fallo abierto.
    pub fn from_env(model: String, timeout_ms: u64) -> Option<JevDecisionsConfig> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .ok()
            .filter(|key| !key.is_empty())?;
        let base_url = std::env::var("ROUTER_BASE_URL")
            .ok()
            .filter(|url| !url.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        Some(JevDecisionsConfig {
            api_key,
            base_url,
            model,
            timeout_ms,
        })
    }
}

/// Cliente HTTP de la Decisions API de Jev.
///
/// Habla `POST {base_url}/alpha/decisions` con preguntas tipadas `noul` y
/// traduce la respuesta (probabilidades + uso) al contrato del trait
/// [`DecisionsProvider`].
pub struct JevDecisionsProvider {
    config: JevDecisionsConfig,
    client: Client,
}

impl JevDecisionsProvider {
    /// Construye el cliente con un timeout propio tomado de `config.timeout_ms`.
    pub fn new(config: JevDecisionsConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .unwrap_or_default();
        Self { config, client }
    }

    /// URL del endpoint, normalizando la barra final de `base_url`.
    fn endpoint(&self) -> String {
        format!(
            "{}/alpha/decisions",
            self.config.base_url.trim_end_matches('/')
        )
    }

    /// Construye el cuerpo con la forma exacta del servicio: `questions` es un
    /// objeto indexado por id y `criteria` lleva las claves `true` y `false`.
    fn build_body(&self, request: &DecisionsRequest) -> Value {
        let questions: serde_json::Map<String, Value> = request
            .questions
            .iter()
            .map(|question| {
                (
                    question.id.clone(),
                    serde_json::json!({
                        "type": "noul",
                        "instructions": question.instructions,
                        "criteria": {
                            "true": question.criteria_true,
                            "false": question.criteria_false,
                        },
                    }),
                )
            })
            .collect();

        serde_json::json!({
            "model": request.model,
            "state": request.state,
            "questions": questions,
        })
    }

    /// Interpreta la respuesta del servicio.
    ///
    /// Solo se leen los ids efectivamente preguntados: cualquier id extra que
    /// venga en `answers` se ignora. Una respuesta sin objeto `answers` o con
    /// una probabilidad `noul` fuera de `[0, 1]` es un error tipado.
    fn parse_response(
        body: &Value,
        questions: &[DecisionsQuestion],
    ) -> Result<DecisionsResponse, LLMError> {
        let answers = body
            .get("answers")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                LLMError::HttpError(
                    "Decisions response is missing the `answers` object".to_string(),
                )
            })?;

        let mut probabilities = HashMap::with_capacity(questions.len());
        for question in questions {
            let Some(answer) = answers.get(&question.id) else {
                continue;
            };
            let noul = answer.get("noul").and_then(Value::as_f64).ok_or_else(|| {
                LLMError::HttpError(format!(
                    "Decisions answer for `{}` is missing `noul`",
                    question.id
                ))
            })?;
            if !(0.0..=1.0).contains(&noul) {
                return Err(LLMError::HttpError(format!(
                    "Decisions probability for `{}` is out of range: {noul}",
                    question.id
                )));
            }
            probabilities.insert(question.id.clone(), noul as f32);
        }

        Ok(DecisionsResponse {
            answers: probabilities,
            input_tokens: body["usage"]["input_tokens"].as_u64().unwrap_or(0),
            output_tokens: body["usage"]["output_tokens"].as_u64().unwrap_or(0),
            cost: body["usage"]["cost"].as_f64().unwrap_or(0.0),
        })
    }
}

#[async_trait]
impl DecisionsProvider for JevDecisionsProvider {
    async fn decide(&self, request: DecisionsRequest) -> Result<DecisionsResponse, LLMError> {
        let url = self.endpoint();
        let body = self.build_body(&request);

        tracing::debug!(
            model = %self.config.model,
            question_count = request.questions.len(),
            "Sending request to the Jev Decisions API"
        );

        let started = std::time::Instant::now();
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", APP_URL)
            .header("X-Title", APP_NAME)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LLMError::Timeout(e.to_string())
                } else {
                    LLMError::HttpError(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let code = status.as_u16();
            let body_text = response.text().await.unwrap_or_default();
            let details = if body_text.is_empty() {
                format!("HTTP {code}")
            } else {
                format!("HTTP {code} — {}", body_text.trim())
            };
            return match code {
                429 => Err(LLMError::RateLimited { retry_after: 30 }),
                401 => Err(LLMError::AuthError(format!("Invalid API key: {details}"))),
                _ => Err(LLMError::HttpError(details)),
            };
        }

        let response_body: Value = response
            .json()
            .await
            .map_err(|e| LLMError::HttpError(format!("Failed to parse Decisions response: {e}")))?;

        let parsed = Self::parse_response(&response_body, &request.questions)?;

        tracing::debug!(
            latency_ms = started.elapsed().as_millis() as u64,
            cost = parsed.cost,
            input_tokens = parsed.input_tokens,
            output_tokens = parsed.output_tokens,
            "Jev Decisions response received"
        );

        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderMap;
    use serial_test::serial;
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn question(id: &str) -> DecisionsQuestion {
        DecisionsQuestion {
            id: id.to_string(),
            instructions: format!("¿hace falta la habilidad {id}?"),
            criteria_true: format!("el turno requiere {id}"),
            criteria_false: format!("el turno no requiere {id}"),
        }
    }

    fn request(questions: Vec<DecisionsQuestion>) -> DecisionsRequest {
        DecisionsRequest {
            model: "typesafe/jev-1.13".to_string(),
            state: serde_json::json!({
                "message": "busca una cafetería y créame un evento",
                "history": ["hola", "buenos días"],
            }),
            questions,
        }
    }

    fn provider(server: &MockServer, timeout_ms: u64) -> JevDecisionsProvider {
        JevDecisionsProvider::new(JevDecisionsConfig {
            api_key: "sk-test-secret".to_string(),
            base_url: server.uri(),
            model: "typesafe/jev-1.13".to_string(),
            timeout_ms,
        })
    }

    fn ok_body(answers: Value) -> Value {
        serde_json::json!({
            "answers": answers,
            "usage": { "input_tokens": 476, "output_tokens": 70, "cost": 0.000019992 },
        })
    }

    // ─── Forma de la petición ───────────────────────────────────────────────

    #[tokio::test]
    async fn request_has_the_shape_of_the_service() {
        let server = MockServer::start().await;
        let captured: Arc<Mutex<Option<(Value, HeaderMap)>>> = Arc::new(Mutex::new(None));
        let sink = captured.clone();

        Mock::given(method("POST"))
            .and(path("/alpha/decisions"))
            .respond_with(move |req: &wiremock::Request| {
                let body = serde_json::from_slice::<Value>(&req.body).ok();
                *sink.lock().unwrap() = Some((body.unwrap_or(Value::Null), req.headers.clone()));
                ResponseTemplate::new(200).set_body_json(ok_body(serde_json::json!({})))
            })
            .mount(&server)
            .await;

        let provider = provider(&server, 5_000);
        provider
            .decide(request(vec![question("agenda"), question("clima")]))
            .await
            .expect("a well-formed response must be Ok");

        let (body, headers) = captured
            .lock()
            .unwrap()
            .clone()
            .expect("the mock must have captured the request");

        assert_eq!(
            body["model"], "typesafe/jev-1.13",
            "the model is the configured one"
        );
        assert_eq!(
            body["state"],
            serde_json::json!({
                "message": "busca una cafetería y créame un evento",
                "history": ["hola", "buenos días"],
            }),
            "the state is forwarded verbatim"
        );

        let questions = body["questions"]
            .as_object()
            .expect("`questions` must be an object indexed by id");
        assert_eq!(questions.len(), 2, "one entry per asked question");

        for id in ["agenda", "clima"] {
            let entry = &questions[id];
            assert_eq!(entry["type"], "noul", "every question is of type `noul`");
            assert_eq!(
                entry["instructions"],
                format!("¿hace falta la habilidad {id}?")
            );
            assert_eq!(entry["criteria"]["true"], format!("el turno requiere {id}"));
            assert_eq!(
                entry["criteria"]["false"],
                format!("el turno no requiere {id}")
            );
        }

        let authorization = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok());
        assert_eq!(
            authorization,
            Some("Bearer sk-test-secret"),
            "the API key travels in the Authorization header"
        );
    }

    #[tokio::test]
    async fn base_url_trailing_slash_is_normalized() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/alpha/decisions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body(serde_json::json!({}))))
            .mount(&server)
            .await;

        let provider = JevDecisionsProvider::new(JevDecisionsConfig {
            api_key: "sk-test-secret".to_string(),
            base_url: format!("{}/", server.uri()),
            model: "typesafe/jev-1.13".to_string(),
            timeout_ms: 5_000,
        });

        provider
            .decide(request(vec![question("agenda")]))
            .await
            .expect("a trailing slash must not break the endpoint path");
    }

    /// The model in the body comes from the request, not the client config.
    #[tokio::test]
    async fn request_model_overrides_the_client_config() {
        let server = MockServer::start().await;
        let captured: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let sink = captured.clone();

        Mock::given(method("POST"))
            .and(path("/alpha/decisions"))
            .respond_with(move |req: &wiremock::Request| {
                let body = serde_json::from_slice::<Value>(&req.body).ok();
                *sink.lock().unwrap() = body;
                ResponseTemplate::new(200).set_body_json(ok_body(serde_json::json!({})))
            })
            .mount(&server)
            .await;

        // `provider` sets config.model = "typesafe/jev-1.13"; the request asks
        // for a different one and that one must travel in the body.
        let provider = provider(&server, 5_000);
        let mut req = request(vec![question("agenda")]);
        req.model = "custom/decisions-2".to_string();

        provider.decide(req).await.expect("a valid response is Ok");

        let body = captured
            .lock()
            .unwrap()
            .clone()
            .expect("the mock must have captured the request");
        assert_eq!(
            body["model"], "custom/decisions-2",
            "the body must carry request.model, not the client config's model"
        );
    }

    // ─── Parseo ─────────────────────────────────────────────────────────────

    #[test]
    fn parses_probabilities_tokens_and_cost() {
        let body = ok_body(serde_json::json!({
            "agenda": { "type": "noul", "noul": 0.96 },
            "clima": { "type": "noul", "noul": 0.12 },
        }));
        let questions = vec![question("agenda"), question("clima")];

        let response = JevDecisionsProvider::parse_response(&body, &questions)
            .expect("a well-formed response must parse");

        assert_eq!(response.answers.len(), 2);
        assert!((response.answers["agenda"] - 0.96).abs() < 1e-6);
        assert!((response.answers["clima"] - 0.12).abs() < 1e-6);
        assert_eq!(response.input_tokens, 476);
        assert_eq!(response.output_tokens, 70);
        assert!((response.cost - 0.000019992).abs() < 1e-12);
    }

    #[test]
    fn unknown_ids_are_ignored_without_error() {
        let body = ok_body(serde_json::json!({
            "agenda": { "type": "noul", "noul": 0.81 },
            "skill_inexistente": { "type": "noul", "noul": 0.99 },
        }));
        let questions = vec![question("agenda")];

        let response = JevDecisionsProvider::parse_response(&body, &questions)
            .expect("an unknown id must not be an error");

        assert_eq!(response.answers.len(), 1, "only asked ids are returned");
        assert!(response.answers.contains_key("agenda"));
        assert!(!response.answers.contains_key("skill_inexistente"));
    }

    // ─── Errores tipados ────────────────────────────────────────────────────

    #[tokio::test]
    async fn http_500_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;

        let result = provider(&server, 5_000)
            .decide(request(vec![question("agenda")]))
            .await;

        assert!(
            matches!(result, Err(LLMError::HttpError(_))),
            "got {result:?}"
        );
    }

    #[tokio::test]
    async fn timeout_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(500))
                    .set_body_json(ok_body(serde_json::json!({
                        "agenda": { "type": "noul", "noul": 0.9 },
                    }))),
            )
            .mount(&server)
            .await;

        let result = provider(&server, 50)
            .decide(request(vec![question("agenda")]))
            .await;

        assert!(
            matches!(result, Err(LLMError::Timeout(_))),
            "got {result:?}"
        );
    }

    #[tokio::test]
    async fn invalid_json_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string("this is not json"))
            .mount(&server)
            .await;

        let result = provider(&server, 5_000)
            .decide(request(vec![question("agenda")]))
            .await;

        assert!(
            matches!(result, Err(LLMError::HttpError(_))),
            "got {result:?}"
        );
    }

    #[test]
    fn out_of_range_probability_is_an_error() {
        let body = ok_body(serde_json::json!({
            "agenda": { "type": "noul", "noul": 1.4 },
        }));
        let questions = vec![question("agenda")];

        let result = JevDecisionsProvider::parse_response(&body, &questions);

        assert!(result.is_err(), "noul=1.4 must be rejected, got {result:?}");
    }

    #[test]
    fn missing_answers_object_is_an_error() {
        let body = serde_json::json!({ "usage": { "input_tokens": 1 } });
        let questions = vec![question("agenda")];

        let result = JevDecisionsProvider::parse_response(&body, &questions);

        assert!(result.is_err(), "a response without `answers` must fail");
    }

    // ─── Construcción desde el entorno ──────────────────────────────────────

    fn restore(var: &str, value: Option<String>) {
        match value {
            Some(value) => std::env::set_var(var, value),
            None => std::env::remove_var(var),
        }
    }

    #[test]
    #[serial]
    fn from_env_without_api_key_is_none() {
        let saved_key = std::env::var("OPENROUTER_API_KEY").ok();
        let saved_url = std::env::var("ROUTER_BASE_URL").ok();
        std::env::remove_var("OPENROUTER_API_KEY");
        std::env::remove_var("ROUTER_BASE_URL");

        let config = JevDecisionsConfig::from_env("typesafe/jev-1.13".to_string(), 800);

        assert!(
            config.is_none(),
            "without an API key there is no classifier"
        );

        restore("OPENROUTER_API_KEY", saved_key);
        restore("ROUTER_BASE_URL", saved_url);
    }

    #[test]
    #[serial]
    fn from_env_with_empty_api_key_is_none() {
        let saved_key = std::env::var("OPENROUTER_API_KEY").ok();
        let saved_url = std::env::var("ROUTER_BASE_URL").ok();
        std::env::set_var("OPENROUTER_API_KEY", "");
        std::env::remove_var("ROUTER_BASE_URL");

        let config = JevDecisionsConfig::from_env("typesafe/jev-1.13".to_string(), 800);

        assert!(
            config.is_none(),
            "an empty API key must be treated as absent so routing fails open"
        );

        restore("OPENROUTER_API_KEY", saved_key);
        restore("ROUTER_BASE_URL", saved_url);
    }

    #[test]
    #[serial]
    fn from_env_reads_the_base_url_from_the_environment() {
        let saved_key = std::env::var("OPENROUTER_API_KEY").ok();
        let saved_url = std::env::var("ROUTER_BASE_URL").ok();
        std::env::set_var("OPENROUTER_API_KEY", "sk-or-v1-test");
        std::env::set_var("ROUTER_BASE_URL", "https://router.example.com/api");

        let config = JevDecisionsConfig::from_env("typesafe/jev-1.13".to_string(), 800)
            .expect("with an API key the classifier is built");

        assert_eq!(config.api_key, "sk-or-v1-test");
        assert_eq!(config.base_url, "https://router.example.com/api");
        assert_eq!(config.model, "typesafe/jev-1.13");
        assert_eq!(config.timeout_ms, 800);

        restore("OPENROUTER_API_KEY", saved_key);
        restore("ROUTER_BASE_URL", saved_url);
    }

    #[test]
    #[serial]
    fn from_env_defaults_the_base_url_when_unset() {
        let saved_key = std::env::var("OPENROUTER_API_KEY").ok();
        let saved_url = std::env::var("ROUTER_BASE_URL").ok();
        std::env::set_var("OPENROUTER_API_KEY", "sk-or-v1-test");
        std::env::remove_var("ROUTER_BASE_URL");

        let config = JevDecisionsConfig::from_env("typesafe/jev-1.13".to_string(), 800)
            .expect("with an API key the classifier is built");

        assert_eq!(config.base_url, "https://openrouter.ai/api");

        restore("OPENROUTER_API_KEY", saved_key);
        restore("ROUTER_BASE_URL", saved_url);
    }
}
