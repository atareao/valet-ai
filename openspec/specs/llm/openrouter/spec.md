# LLM: OpenRouter SSE Streaming — Finish Reason Handling

## Purpose

Contrato del proveedor OpenRouter: parsing de respuestas y eventos SSE, acumulación y finalización de tool calls, extracción del coste y cabeceras de identificación de la aplicación.

## Contracts

### `parse_sse_event` behavior

| Input | Current behavior | Expected behavior |
|---|---|---|
| `delta: {}, finish_reason: "tool_calls"` | Returns `Ok(None)` — no event emitted | Returns `Ok(Some(StreamEvent::Done(...)))` with accumulated tool calls |
| `delta: {tool_calls: [...]}, finish_reason: "tool_calls"` (single tool call) | Returns `Ok(Some(StreamEvent::ToolCall(...)))` — only first tool call | Returns `Ok(Some(StreamEvent::Done(...)))` **with all** accumulated tool calls |
| `delta: {tool_calls: [...]}, finish_reason: "tool_calls"` (multiple tool calls) | Returns one `ToolCall` for the first, rest lost | Returns `Done` with **all** accumulated tool calls |
| `delta: {}, finish_reason: "stop"` | Returns `Ok(Some(StreamEvent::Done(...)))` | Same (unchanged) |

### `StreamAccumulator.finalize()` — new method

```rust
impl StreamAccumulator {
    /// Consume all partial tool calls and produce a Vec<ToolCall>.
    /// Returns None if no tool calls were accumulated.
    pub fn finalize(&mut self) -> Option<Vec<ToolCall>>;
}
```

### Application Identification Headers

```rust
const APP_NAME: &str = "Valet";
const APP_URL: &str = "https://github.com/atareao/valet-ai";
```

Los métodos `chat()` y `chat_stream()` de `OpenRouterProvider` (en `src/llm/openrouter.rs`), así como `embed()` de `embeddings::OpenRouterProvider` (en `src/embeddings/openrouter.rs`), añaden los headers `HTTP-Referer` y `X-Title` a todas las peticiones HTTP a OpenRouter.

### `parse_response` content extraction

`OpenRouterProvider::parse_response()` extrae `content` del mensaje con la siguiente lógica:
1. Si `message["content"]` es un string → se usa directamente.
2. Si `message["content"]` es un array de partes (formato OpenAI multi-modal) → se concatenan todos los campos `text`.
3. Si `message["content"]` es `null` → se usa string vacío.

### `ChatRequest.model` SHALL be respected

`OpenRouterProvider::chat()` y `chat_stream()` SHALL usar `request.model` cuando no esté vacío, y caer en `self.config.model` solo como fallback.

## Scenarios

### Scenario 1: finish_reason "tool_calls" without tool_calls in delta

**Given** an SSE line with `{"choices":[{"delta":{},"finish_reason":"tool_calls","index":0}]}`  
**When** `parse_sse_event` processes it  
**Then** it returns `Ok(Some(StreamEvent::Done(...)))`  
**And** the Done response contains `tool_calls: None`

### Scenario 2: finish_reason "tool_calls" with accumulated tool calls

**Given** a StreamAccumulator that has a partial tool call stored  
**And** an SSE line with `{"choices":[{"delta":{},"finish_reason":"tool_calls","index":0}]}`  
**When** `parse_sse_event` processes it  
**Then** it returns `Ok(Some(StreamEvent::Done(...)))`  
**And** the Done response contains `tool_calls: Some([...])` with the accumulated tool calls  
**And** the arguments string is parsed as JSON

### Scenario 3: finish_reason "tool_calls" with multiple tool calls

**Given** a StreamAccumulator that has 2 partial tool calls stored  
**And** an SSE line with `{"choices":[{"delta":{},"finish_reason":"tool_calls","index":0}]}`  
**When** `parse_sse_event` processes it  
**Then** it returns `Ok(Some(StreamEvent::Done(...)))`  
**And** the Done response contains `tool_calls: Some([tool_call_1, tool_call_2])`

### Scenario 4: finish_reason "stop" still works (regression guard)

**Given** an SSE line with `{"choices":[{"delta":{},"finish_reason":"stop","index":0}],"usage":{...}}`  
**When** `parse_sse_event` processes it  
**Then** it returns `Ok(Some(StreamEvent::Done(...)))`  
**And** the Done response contains usage information  
**And** this is unchanged from current behavior

### Scenario 5: chat() sends application identification headers

**Given** un `OpenRouterProvider` configurado  
**When** se llama a `chat()` con un `ChatRequest` válido  
**Then** la petición HTTP incluye los headers `HTTP-Referer: https://github.com/atareao/valet-ai` y `X-Title: Valet`

### Scenario 6: chat_stream() sends application identification headers

**Given** un `OpenRouterProvider` configurado  
**When** se llama a `chat_stream()` con un `ChatRequest` válido  
**Then** la petición HTTP incluye los headers `HTTP-Referer` y `X-Title`

### Scenario 8: embed() (EmbeddingProvider) sends application identification headers

**Given** un `embeddings::OpenRouterProvider` configurado  
**When** se llama a `embed()` con un texto  
**Then** la petición HTTP incluye los headers `HTTP-Referer` y `X-Title`

### Scenario 9: parse_response handles content as array of parts

**Given** a response body where `choices[0].message.content` is `[{"type":"text","text":"Part 1 "},{"type":"text","text":"part 2"}]`  
**When** `parse_response()` processes it  
**Then** the resulting `ChatResponse.message.content` SHALL be `"Part 1 part 2"`

### Scenario 10: parse_response handles content as plain string

**Given** a response body where `choices[0].message.content` is `"Hello"`  
**When** `parse_response()` processes it  
**Then** the resulting `ChatResponse.message.content` SHALL be `"Hello"`

### Scenario 11: parse_response handles content as null

**Given** a response body where `choices[0].message.content` is `null` and `tool_calls` is present  
**When** `parse_response()` processes it  
**Then** the resulting `ChatResponse.message.content` SHALL be `""`  
**And** `tool_calls` SHALL be `Some(...)`

## Requirements

### Requirement: Trace logging for cost debugging

`OpenRouterProvider::chat()` y `chat_stream()` SHALL loguear a nivel TRACE el JSON crudo de las respuestas
para diagnosticar si OpenRouter incluye el campo `cost` en el objeto `usage`.

En el streaming path (`chat_stream`):
- Cada línea SSE raw SHALL loguearse con `tracing::trace!(raw_line = %trimmed, "OpenRouter SSE raw")`
- Cuando se procesa `finish_reason` con `usage`, el body JSON completo SHALL loguearse con `tracing::trace!(usage_json = %body_str, "OpenRouter usage chunk")`

En el non-streaming path (`chat`):
- El body JSON completo de la respuesta SHALL loguearse con `tracing::trace!(raw_response = %raw_json, "OpenRouter raw response body")`

#### Scenario: Trace logging outputs raw SSE and usage JSON
- **WHEN** `chat_stream` processes an SSE line with `finish_reason` and `usage`
- **THEN** it SHALL log the raw line at TRACE level with message "OpenRouter SSE raw"
- **AND** it SHALL log the full usage JSON at TRACE level with message "OpenRouter usage chunk"

### Requirement: Cost retrieval from OpenRouter response

`parse_sse_event()` SHALL extraer `cost` del campo `usage.cost` en la respuesta JSON de OpenRouter.
Si el campo no está presente o es `null`, SHALL devolver `0.0` (no romper la funcionalidad).

`parse_response()` SHALL extraer `cost` del campo `usage.cost` en la respuesta JSON de OpenRouter.
Si el campo no está presente o es `null`, SHALL devolver `0.0`.

#### Scenario: Cost is extracted from usage.cost
- **WHEN** the OpenRouter response contains `usage.cost`
- **THEN** the parsed `TokenUsage` SHALL contain that cost value
- **AND** a missing or null cost SHALL default to `0.0`

### Requirement: OpenRouterProvider SHALL NOT implement embed on LLMProvider

`OpenRouterProvider` SHALL NOT implementar `embed` como parte de `LLMProvider`. La generación de embeddings de OpenRouter SHALL realizarse exclusivamente a través de `embeddings::OpenRouterProvider`, que mantiene los headers de identificación de la aplicación.

**Given** `OpenRouterProvider` en `src/llm/openrouter.rs`  
**When** se inspecciona su impl de `LLMProvider`  
**Then** SHALL NOT contener `async fn embed`  
**And** el embedding de OpenRouter SHALL generarse vía `embeddings::OpenRouterProvider`  
**And** `embeddings::OpenRouterProvider::embed()` SHALL seguir enviando los headers `HTTP-Referer` y `X-Title`

#### Scenario: El provider LLM no implementa embed
**Given** el impl `LLMProvider for OpenRouterProvider`  
**When** se inspecciona  
**Then** no contiene `async fn embed`

#### Scenario: El provider de embeddings mantiene los headers
**Given** un `embeddings::OpenRouterProvider` configurado  
**When** se llama a `embed()`  
**Then** la petición HTTP incluye `HTTP-Referer: https://github.com/atareao/valet-ai` y `X-Title: Valet`

### Requirement: OpenRouterProvider SHALL forward reasoning and response_format

`OpenRouterProvider::chat()` y `OpenRouterProvider::chat_stream()` SHALL incluir en el cuerpo de la
petición las claves `reasoning` y `response_format` cuando el `ChatRequest` las traiga, con la
forma exacta que documenta OpenRouter, y SHALL omitirlas cuando sean `None`. `reasoning` SHALL
serializarse como `{ "enabled": false }` para `Off` y `{ "effort": "<nivel>" }` para `Effort`.
`response_format` SHALL serializarse como `{ "type": "json_object" }`. El reenvío SHALL ser
idéntico en el camino síncrono y en el de streaming.

**Given** un `ChatRequest` con `reasoning = Some(ReasoningSpec::Off)`  
**When** `OpenRouterProvider::chat()` construye el body  
**Then** `body["reasoning"]` SHALL ser `{ "enabled": false }`

**Given** un `ChatRequest` con `reasoning = Some(ReasoningSpec::Effort(ReasoningEffort::High))`  
**When** `OpenRouterProvider::chat()` construye el body  
**Then** `body["reasoning"]` SHALL ser `{ "effort": "high" }`

**Given** un `ChatRequest` con `response_format = Some(ResponseFormat::JsonObject)`  
**When** `OpenRouterProvider::chat()` construye el body  
**Then** `body["response_format"]` SHALL ser `{ "type": "json_object" }`

**Given** un `ChatRequest` con ambos campos en `None`  
**When** `OpenRouterProvider::chat()` construye el body  
**Then** SHALL NOT existir la clave `reasoning`  
**And** SHALL NOT existir la clave `response_format`

#### Scenario: chat() reenvía reasoning desactivado
**Given** un `ChatRequest` con `reasoning = Some(ReasoningSpec::Off)`  
**When** `chat()` construye el body  
**Then** el body contiene `"reasoning": { "enabled": false }`

#### Scenario: chat() reenvía un nivel de razonamiento
**Given** un `ChatRequest` con `reasoning = Some(ReasoningSpec::Effort(ReasoningEffort::Low))`  
**When** `chat()` construye el body  
**Then** el body contiene `"reasoning": { "effort": "low" }`

#### Scenario: chat() reenvía el modo JSON
**Given** un `ChatRequest` con `response_format = Some(ResponseFormat::JsonObject)`  
**When** `chat()` construye el body  
**Then** el body contiene `"response_format": { "type": "json_object" }`

#### Scenario: campos ausentes no aparecen en el body
**Given** un `ChatRequest` sin `reasoning` ni `response_format`  
**When** `chat()` construye el body  
**Then** el body NO contiene las claves `reasoning` ni `response_format`

#### Scenario: chat_stream() reenvía reasoning igual que chat()
**Given** un `ChatRequest` con `reasoning = Some(ReasoningSpec::Effort(ReasoningEffort::High))` y `stream: true`  
**When** `chat_stream()` construye el body  
**Then** el body contiene `"reasoning": { "effort": "high" }`  
**And** contiene `"stream": true`

#### Scenario: chat_stream() omite los campos cuando son None
**Given** un `ChatRequest` con `stream: true` y ambos campos en `None`  
**When** `chat_stream()` construye el body  
**Then** el body NO contiene las claves `reasoning` ni `response_format`

## Scenarios

### Scenario 12: Streaming usage chunk includes cost

**Given** una línea SSE con `{"choices":[{"delta":{},"finish_reason":"stop","index":0}],"usage":{"prompt_tokens":10,"completion_tokens":5,"cost":0.0015}}`  
**When** `parse_sse_event` la procesa  
**Then** devuelve `Ok(Some(StreamEvent::Done(...)))`  
**And** el `TokenUsage` contiene `cost = 0.0015`

### Scenario 13: Streaming usage chunk without cost

**Given** una línea SSE con `{"choices":[{"delta":{},"finish_reason":"stop","index":0}],"usage":{"prompt_tokens":10,"completion_tokens":5}}`  
**When** `parse_sse_event` la procesa  
**Then** devuelve `Ok(Some(StreamEvent::Done(...)))`  
**And** el `TokenUsage` contiene `cost = 0.0`

### Scenario 14: Non-streaming response without cost

**Given** un body JSON de respuesta sin campo `usage.cost`  
**When** `parse_response` lo procesa  
**Then** devuelve un `ChatResponse` con `usage.cost = 0.0`

### Scenario 15: Trace logging outputs raw JSON

**Given** una línea SSE con contenido JSON  
**When** `chat_stream` la recibe  
**Then** se loguea a nivel TRACE: `"OpenRouter SSE raw"` con el contenido de la línea  
**And** si contiene `finish_reason` + `usage`, se loguea también `"OpenRouter usage chunk"` con el JSON completo
