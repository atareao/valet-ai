# Spec Delta: workers

## ADDED Requirements

### Requirement: El worker SHALL escribir la Capa B y la Capa C en la misma pasada y con una sola marca

Al procesar un lote de mensajes sin indexar, el `EpisodicMemoryWorker` SHALL: (1) leer el lote
una sola vez; (2) obtener la ficha episódica (Capa B) y el estado persistente consolidado
(Capa C) **antes** de escribir nada; y (3) escribir la ficha, escribir el estado y marcar los
mensajes como indexados en una **única transacción**. `messages.is_indexed = 1` SHALL implicar
que la ficha (Capa B) se ha escrito y que existe un estado de Capa C válido —el recién
consolidado o, si el tamaño obligó a conservarlo, el anterior—.

**Given** un lote de mensajes sin indexar que cumple las condiciones de procesamiento  
**When** el worker lo procesa  
**Then** SHALL obtener la ficha episódica y el estado persistente consolidado del mismo lote  
**And** SHALL escribir ambos y marcar los mensajes en una única transacción  
**And** tras el commit, `is_indexed` SHALL ser `1` para los mensajes del lote

#### Scenario: Un lote produce ficha y estado, y marca en una transacción
**Given** un lote de mensajes sin indexar que cumple las condiciones  
**When** el worker lo procesa y ambas extracciones tienen éxito  
**Then** se persiste una ficha episódica  
**And** se persiste el estado persistente  
**And** los mensajes del lote pasan a `is_indexed = 1` en la misma transacción

#### Scenario: Sin mensajes sin indexar no hay escritura ni marca
**Given** una base de datos sin mensajes con `is_indexed = 0`  
**When** el worker evalúa  
**Then** NO SHALL escribir ninguna ficha ni estado  
**And** NO SHALL llamar al LLM

#### Scenario: El rechazo por techo conserva el estado previo y marca el lote
**Given** un lote cuya Capa C válida supera el techo absoluto  
**When** el worker procesa  
**Then** se persiste la ficha episódica  
**And** NO se sobrescribe el estado persistente (se conserva el anterior)  
**And** los mensajes del lote pasan a `is_indexed = 1`

### Requirement: Un fallo en cualquiera de las dos extracciones SHALL NOT dejar escritura parcial ni marca

Si falla la llamada episódica, la llamada de consolidación, la validación del estado o la
generación de embeddings, el worker SHALL NOT abrir transacción, SHALL NOT marcar los mensajes y SHALL
iniciar el cooldown. Al reintentar, SHALL reprocesar el lote completo sin duplicar fichas.

**Given** un lote cuya extracción episódica falla  
**When** el worker lo procesa  
**Then** SHALL NOT escribir nada  
**And** SHALL NOT marcar los mensajes  
**And** SHALL iniciar el cooldown

**Given** un lote cuya consolidación del estado falla tras una extracción episódica correcta  
**When** el worker lo procesa  
**Then** SHALL NOT escribir ni la ficha ni el estado  
**And** SHALL NOT marcar los mensajes  
**And** un reintento posterior SHALL producir una única ficha, sin duplicados

#### Scenario: Fallo episódico no escribe ni marca
**Given** un lote y una llamada episódica que falla  
**When** el worker procesa  
**Then** `memory` no gana filas  
**And** `messages.is_indexed` sigue en `0` para el lote  
**And** el cooldown queda activo

#### Scenario: Fallo del consolidador no deja ficha huérfana ni duplicados
**Given** un lote, una extracción episódica correcta y una consolidación que falla  
**When** el worker procesa  
**Then** NO se persiste la ficha  
**And** NO se persiste el estado  
**And** los mensajes siguen sin marcar  
**And** al reintentar con éxito se persiste exactamente una ficha

### Requirement: El modelo del consolidador SHALL ser configurable por variable de entorno y registrar stats

`Config::from_env()` SHALL leer `SEMANTIC_MODEL` y, si no está definida, SHALL usar el valor
de `MEMORY_MODEL`. La llamada de consolidación SHALL usar ese modelo en `ChatRequest.model`.
La llamada SHALL registrar stats con `profile_id` NULL, como las demás llamadas de worker.

**Given** las variables de entorno  
**When** se construye la configuración  
**Then** `semantic_model` SHALL tomar `SEMANTIC_MODEL` si existe  
**And** SHALL caer a `MEMORY_MODEL` si no existe

#### Scenario: Modelo por defecto cae a MEMORY_MODEL
**Given** `MEMORY_MODEL = "mistralai/mistral-small-24b-instruct-2501"` y sin `SEMANTIC_MODEL`  
**When** se construye la configuración  
**Then** `semantic_model` es `"mistralai/mistral-small-24b-instruct-2501"`

#### Scenario: Modelo personalizado del consolidador
**Given** `SEMANTIC_MODEL = "google/gemini-2.0-flash-lite"`  
**When** se construye la configuración  
**Then** `semantic_model` es `"google/gemini-2.0-flash-lite"`

#### Scenario: Stats del consolidador con profile NULL
**Given** una consolidación que llama al LLM  
**When** se registra la petición  
**Then** `llm_requests.profile_id` es `NULL`
