# Spec Delta: llm/openrouter

## ADDED Requirements

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
