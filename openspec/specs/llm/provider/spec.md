# llm/provider Specification

## Purpose
Contrato de los proveedores LLM de Valet (OpenRouter y Ollama): parsing de tool calls en las respuestas y emisión de eventos SSE reales en el streaming de chat.

## Requirements

### Requirement: Parse tool_calls from OpenRouter responses

`OpenRouterProvider::chat()` SHALL parsear `choices[0].message.tool_calls` a `ChatResponse.message.tool_calls`, con `id`, `name` y `arguments` como `Value`.
**Given** una respuesta de OpenRouter con `choices[0].message.tool_calls`  
**When** `OpenRouterProvider::chat()` procesa la respuesta  
**Then** devuelve `ChatResponse` con `message.tool_calls` conteniendo los tool calls  
**And** cada tool call tiene `id`, `name` y `arguments` como `Value`  
**And** `content` puede ser `""` (string vacío) si el LLM devolvió `null`

#### Scenario: OpenRouter tool_calls se parsean correctamente
**Given** una respuesta de OpenRouter con `choices[0].message.tool_calls`  
**When** `OpenRouterProvider::chat()` procesa la respuesta  
**Then** devuelve `ChatResponse` con `message.tool_calls` conteniendo los tool calls  
**And** cada tool call tiene `id`, `name` y `arguments` como `Value`  
**And** `content` puede ser `""` (string vacío) si el LLM devolvió `null`

#### Scenario: OpenRouter respuesta sin tool_calls
**Given** una respuesta de OpenRouter sin `tool_calls`  
**When** `OpenRouterProvider::chat()` procesa la respuesta  
**Then** devuelve `ChatResponse` con `message.tool_calls == None`  
**And** `message.content` contiene el texto de la respuesta

#### Scenario: OpenRouter arguments es string JSON válido
**Given** una respuesta de OpenRouter donde `function.arguments` es un string JSON  
**When** se parsea el tool call  
**Then** el string se parsea a `serde_json::Value`  
**And** si el parseo falla, se usa el string original como `Value::String`

### Requirement: Parse tool_calls from Ollama responses

`OllamaProvider::chat()` SHALL parsear `message.tool_calls` a `ChatResponse.message.tool_calls`, generando `id` si no viene.
**Given** una respuesta de Ollama con `message.tool_calls`  
**When** `OllamaProvider::chat()` procesa la respuesta  
**Then** devuelve `ChatResponse` con `message.tool_calls` conteniendo los tool calls  
**And** cada tool call tiene `id` (generado si no viene), `name` y `arguments` como `Value`

#### Scenario: Ollama tool_calls se parsean correctamente
**Given** una respuesta de Ollama con `message.tool_calls`  
**When** `OllamaProvider::chat()` procesa la respuesta  
**Then** devuelve `ChatResponse` con `message.tool_calls` conteniendo los tool calls  
**And** cada tool call tiene `id` (generado si no viene), `name` y `arguments` como `Value`

#### Scenario: Ollama respuesta sin tool_calls
**Given** una respuesta de Ollama sin `tool_calls`  
**When** `OllamaProvider::chat()` procesa la respuesta  
**Then** devuelve `ChatResponse` con `message.tool_calls == None`  
**And** `message.content` contiene el texto de la respuesta

#### Scenario: Ollama arguments es objeto JSON directamente
**Given** una respuesta de Ollama donde `function.arguments` es un objeto JSON  
**When** se parsea el tool call  
**Then** se usa directamente como `Value` sin parseo adicional

### Requirement: OpenRouter chat_stream produces real SSE events

`OpenRouterProvider::chat_stream()` SHALL enviar `stream: true` y emitir eventos SSE reales de chunk, tool call y done.

**Given** un `OpenRouterProvider` configurado  
**When** se llama a `chat_stream()` con `ChatRequest { stream: true }`  
**Then** envía `"stream": true` en el body de la petición a OpenRouter  
**And** parsea el stream SSE línea por línea  
**And** emite `StreamEvent::Chunk` por cada delta de `content`  
**And** emite `StreamEvent::Done` con el `ChatResponse` final incluyendo `usage`  
**And** si aparecen `tool_calls` en el stream, emite `StreamEvent::ToolCall`

#### Scenario: Stream de texto plano (sin tool calls)
**Given** una respuesta SSE de OpenRouter con solo `choices[0].delta.content`  
**When** `chat_stream()` procesa el stream  
**Then** emite `StreamEvent::Chunk(texto)` por cada fragmento  
**And** emite `StreamEvent::Done(ChatResponse { message.content: texto_completo, tool_calls: None })` al final  
**And** el `ChatResponse` final incluye `usage` con tokens del último evento

#### Scenario: Stream con tool call en delta
**Given** una respuesta SSE de OpenRouter  
**When** `choices[0].delta.tool_calls` aparece en el stream  
**Then** acumula los tool calls parcialmente (pueden venir en múltiples chunks)  
**And** al recibir `finish_reason: "tool_calls"`, emite `StreamEvent::ToolCall`  
**And** emite `StreamEvent::Done` con el `ChatResponse` incluyendo los tool calls

#### Scenario: Error en el stream HTTP
**Given** una petición SSE a OpenRouter  
**When** la conexión HTTP falla (timeout, reset, etc.)  
**Then** retorna `Err(LLMError::HttpError)`  
**And** cierra el stream

#### Scenario: chunk vacío se ignora
**Given** un stream SSE de OpenRouter  
**When** se recibe un `data: {}` sin content ni tool_calls  
**Then** no emite ningún evento  
**And** continúa con el siguiente chunk

#### Scenario: chat_stream con stream=false
**Given** un `ChatRequest` con `stream: false`  
**When** se llama a `chat_stream()`  
**Then** el comportamiento puede ser equivalente a `chat()`  
**And** emite un único `StreamEvent::Done` con la respuesta completa

### Requirement: LLMProvider SHALL NOT expose an embed method

El trait `LLMProvider` SHALL NOT declarar un método `embed`. La generación de embeddings SHALL gestionarse exclusivamente a través del trait `EmbeddingProvider` del módulo `embeddings`.

**Given** el trait `LLMProvider`  
**When** se define  
**Then** SHALL NOT declarar `embed`  
**And** los providers `OpenRouterProvider`, `OllamaProvider` y `FallbackProvider` SHALL NOT implementar `embed`  
**And** los embeddings SHALL gestionarse exclusivamente vía `EmbeddingProvider`

#### Scenario: El trait no declara embed
**Given** el código fuente de `src/llm/provider.rs`  
**When** se inspecciona el trait `LLMProvider`  
**Then** no contiene `async fn embed`

#### Scenario: Los providers no implementan embed
**Given** los impls de `LLMProvider` en `openrouter.rs`, `ollama.rs` y `fallback.rs`  
**When** se inspeccionan  
**Then** ninguno define `async fn embed`

### Requirement: ChatRequest SHALL carry optional reasoning and response_format

`ChatRequest` SHALL declarar dos campos opcionales nuevos: `reasoning: Option<ReasoningSpec>` y
`response_format: Option<ResponseFormat>`. `None` SHALL significar «no enviar la clave», nunca
«enviar un objeto vacío». `ReasoningSpec` SHALL ser un enum con dos variantes —`Off` y
`Effort(ReasoningEffort)`— y `ReasoningEffort` SHALL aceptar `Minimal`, `Low`, `Medium`, `High`,
`XHigh` y `Max`. `ResponseFormat` SHALL aceptar, por ahora, `JsonObject`. La serialización de
`ReasoningSpec::Off` SHALL ser `{ "enabled": false }`; la de `ReasoningSpec::Effort(e)` SHALL ser
`{ "effort": "<e>" }` con el nivel en minúsculas; la de `ResponseFormat::JsonObject` SHALL ser
`{ "type": "json_object" }`.

**Given** el tipo `ChatRequest`  
**When** se construye sin `reasoning` ni `response_format`  
**Then** ambos campos SHALL ser `None`  
**And** los providers SHALL omitir ambas claves del cuerpo de la petición

**Given** un `ReasoningSpec::Off`  
**When** se serializa  
**Then** el JSON SHALL ser `{ "enabled": false }`

**Given** un `ReasoningSpec::Effort(ReasoningEffort::Low)`  
**When** se serializa  
**Then** el JSON SHALL ser `{ "effort": "low" }`

**Given** un `ResponseFormat::JsonObject`  
**When** se serializa  
**Then** el JSON SHALL ser `{ "type": "json_object" }`

#### Scenario: ChatRequest por defecto omite los campos
**Given** un `ChatRequest` construido sin los campos nuevos  
**When** se inspecciona  
**Then** `reasoning` es `None`  
**And** `response_format` es `None`

#### Scenario: Off se serializa a enabled=false
**Given** `reasoning = Some(ReasoningSpec::Off)`  
**When** se serializa el campo  
**Then** el JSON resultante es `{ "enabled": false }`  
**And** NO contiene la clave `effort`

#### Scenario: Un nivel se serializa a effort en minúsculas
**Given** `reasoning = Some(ReasoningSpec::Effort(ReasoningEffort::High))`  
**When** se serializa el campo  
**Then** el JSON resultante es `{ "effort": "high" }`

#### Scenario: JsonObject se serializa a type=json_object
**Given** `response_format = Some(ResponseFormat::JsonObject)`  
**When** se serializa el campo  
**Then** el JSON resultante es `{ "type": "json_object" }`

### Requirement: Providers that cannot honour the new fields SHALL ignore them

Un provider que no soporte `reasoning` o `response_format` SHALL ignorar los campos en lugar de
fallar la petición. Los tests de contrato SHALL poder distinguir «ignorar» de «enviar en blanco».

**Given** un `ChatRequest` con `reasoning` y `response_format` definidos  
**When** se usa `OllamaProvider::chat()`  
**Then** la petición SHALL completarse sin error  
**And** SHALL NOT incluir las claves `reasoning` ni `response_format` en el cuerpo

#### Scenario: Ollama ignora reasoning y response_format
**Given** un `ChatRequest` con `reasoning = Some(ReasoningSpec::Off)` y
`response_format = Some(ResponseFormat::JsonObject)`  
**When** `OllamaProvider::chat()` construye el cuerpo de la petición  
**Then** el cuerpo NO contiene `reasoning` ni `response_format`  
**And** la petición no falla por campos desconocidos
