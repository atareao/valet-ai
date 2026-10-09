# Spec Delta: llm/provider

## ADDED Requirements

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
