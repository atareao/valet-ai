# Spec Delta: orchestrator/agent

## REMOVED Requirements

### Requirement: System prompt template uses Markdown and emojis

### Requirement: Orchestrator records stats after each LLM call

### Requirement: Inyección automática del bloque de memoria episódica

## ADDED Requirements

### Requirement: System prompt template comes from settings with Markdown and emojis

**Given** una base de datos migrada con el prompt sembrado en `settings.system_prompt`  
**When** el orquestador construye la petición al LLM  
**Then** SHALL leer `system_prompt` de la tabla `settings`  
**And** SHALL usar ese valor como mensaje de sistema  
**And** SHALL NOT usar ningún template hardcodeado en `OrchestratorConfig`  
**And** el prompt SHALL contener instrucciones de Markdown, "Emojis" y formato rico  
**And** si `system_prompt` está ausente o vacío, SHALL usar un fallback mínimo genérico y loguear un warning

#### Scenario: System prompt incluye personaje de mayordomo
**Given** la base de datos migrada  
**When** se lee `settings.system_prompt`  
**Then** contiene "asistente personal británico"  
**And** contiene "usted"  
**And** contiene "caballero"

#### Scenario: System prompt tiene modo conciso y expandido
**Given** la base de datos migrada  
**When** se lee `settings.system_prompt`  
**Then** contiene "Modo Conciso (Predeterminado)"  
**And** contiene "Expandido"

#### Scenario: System prompt permite Markdown completo y emojis
**Given** la base de datos migrada  
**When** se lee `settings.system_prompt`  
**Then** contiene "Markdown"  
**And** contiene "Emojis"

#### Scenario: process_message_stream usa el prompt de la BD
**Given** un orquestador con `settings.system_prompt = "Prompt de prueba"`  
**When** se llama `process_message_stream()`  
**Then** el mensaje de sistema enviado al LLM es "Prompt de prueba"

#### Scenario: Fallback mínimo si falta el prompt
**Given** un orquestador cuya tabla `settings` no tiene `system_prompt`  
**When** se construye la petición al LLM  
**Then** se usa un fallback mínimo genérico no vacío  
**And** se loguea un warning indicando que falta el prompt

### Requirement: Orchestrator records stats after each streamed LLM call and propagates errors

The Orchestrator SHALL call `StatsRepo::record_request()` after each LLM call in `process_message_stream()`. When the call fails, it SHALL record a row with status `"error"` **and** SHALL propagate the failure to the caller: recording SHALL NOT swallow, replace or mask the error.

#### Scenario: process_message_stream records stats
- **WHEN** `process_message_stream()` receives `StreamEvent::Done`
- **THEN** there is at least one row in `llm_requests` with tokens

#### Scenario: LLM error records stats with error status and propagates
- **WHEN** a mock LLM whose stream always fails is used with `process_message_stream()`
- **THEN** there is a row in `llm_requests` with status = "error" and non-empty error_message
- **AND** `process_message_stream()` returns an error, so the failure is not swallowed

### Requirement: Inyección automática del bloque de memoria episódica en la ruta de streaming

**Given** un mensaje del usuario  
**When** el orquestador construye la petición al LLM  
**Then** SHALL recuperar memorias episódicas por similitud semántica en **cada** mensaje, con independencia de la estrategia de contexto  
**And** SHALL componer el bloque de memoria en código, no desde un placeholder de `settings.system_prompt`  
**And** SHALL omitir el bloque por completo cuando no haya ninguna ficha que supere el umbral, sin texto de relleno del tipo "no hay antecedentes"

#### Scenario: Se inyecta memoria cuando hay fichas sobre el umbral
**Given** un orquestador con pool, provider y al menos una ficha que supera `SIMILARITY_THRESHOLD`  
**When** se construye la petición  
**Then** el array de mensajes contiene el bloque de memoria episódica  
**And** el bloque incluye el contenido de esa ficha

#### Scenario: El bloque se omite por completo sin fichas
**Given** un orquestador con pool y provider pero sin fichas que superen el umbral  
**When** se construye la petición  
**Then** el array de mensajes NO contiene ningún mensaje de memoria episódica  
**And** NO aparece ningún texto de relleno del tipo "no hay antecedentes"

#### Scenario: El bloque no depende de un placeholder editable
**Given** un `settings.system_prompt` sin ningún placeholder de memoria  
**When** se construye la petición  
**Then** el bloque de memoria se compone igualmente en código  
**And** borrar o editar `system_prompt` NO desactiva la memoria
