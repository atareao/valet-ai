# Proposal: Parámetros de generación por rol (temperatura, razonamiento y tokens)

## Why

Hoy el comportamiento generativo se decide de tres formas pobres. La temperatura de los workers
está **hardcodeada** (0.2 el consolidador, 0.3 las fichas y el colapso); la del chat **no se
envía** y queda al criterio del proveedor (a menudo 1.0); y el **nivel de razonamiento no se
controla en absoluto**. Con los modelos actuales —todos razonadores, `deepseek-v4.1-flash` y
`deepseek-v4-flash`— eso tiene dos costes reales: los workers mecánicos pagan tokens de
razonamiento que no necesitan, y esos mismos tokens, dentro de `max_tokens`, pueden **truncar el
JSON** del consolidador. Además, el consolidador confía solo en que el prompt diga «EXCLUSIVAMENTE
JSON», sin `response_format`.

Y estos valores no se pueden tocar sin recompilar. Se quiere que **todo** (temperatura,
razonamiento y tokens) sea **editable desde la UI**, con los valores por defecto ya fijados, y que
el cambio surta efecto en caliente. Eso pide el patrón que ya usan los mandos de memoria: vivir en
`settings` y leerse en cada llamada.

## What Changes

- **D1 — Contrato.** `ChatRequest` gana dos campos opcionales: `reasoning` y `response_format`.
  Tipados (`ReasoningSpec`, `ReasoningEffort`, `ResponseFormat`), no `Value`, para poder probarlos.
- **D2 — Provider.** `OpenRouterProvider` reenvía ambos campos, en `chat()` y `chat_stream()`, con
  la forma exacta de OpenRouter (`reasoning: {effort}` / `{enabled:false}`; `response_format:
  {type:"json_object"}`). Ollama los ignora, en lugar de fallar.
- **D3 — Settings + UI.** Los parámetros de generación (temperatura, razonamiento y tokens máximos,
  por rol) viven en la tabla `settings`, sembrados por migración y **leídos en cada llamada**, de
  modo que cambiarlos surta efecto sin reiniciar. Se editan en una pestaña nueva «Generación» del
  diálogo de ajustes, reutilizando `GET/PUT /settings` (sin rutas nuevas).
- **D4 — Defaults.** Cuatro roles, con estos valores iniciales:

  | Rol | Claves (`settings`) | temp. | razonamiento | tokens |
  |---|---|---|---|---|
  | Chat | `GENERATION_CHAT_*` | 0.7 | *(vacío = default del modelo)* | 4096 |
  | Colapso | `GENERATION_COLLAPSE_*` | 0.2 | `off` | 1024 |
  | Fichas | `GENERATION_MEMORY_*` | 0.3 | `off` | 1024 |
  | Consolidador / compresión | `GENERATION_SEMANTIC_*` | 0.1 | `low` | 2048 |

- **D5 — Sin razonamiento en tareas mecánicas.** Colapso y fichas envían `reasoning: Off` por
  defecto; el consolidador `Effort(Low)`; el chat no envía `reasoning` salvo que se configure.
- **D6 — Modo JSON incondicional.** El consolidador y la pasada de compresión envían siempre
  `response_format: {type:"json_object"}`; **no** es un ajuste, es fiabilidad.
- **D7 — Migración.** Una migración siembra las doce claves con sus defaults, respetando cualquier
  valor ya existente (solo rellena si falta o está vacío).

### Fuera de alcance

- Elegir los **modelos** desde la UI: siguen en variables de entorno. Solo los parámetros de
  generación pasan a `settings`.
- Structured output en el chat: solo consolidador y compresión.
- La reflexión del agente (`analyze()`) conserva sus propios valores (0.3 / 256): queda fuera de la configuración por rol.
- Tocar `docker-compose.prod.yml`.

## Capabilities

### New Capabilities

- (ninguna)

### Modified Capabilities

- `llm/provider`: el contrato de `ChatRequest` gana `reasoning` y `response_format`.
- `llm/openrouter`: reenvío de `reasoning` y `response_format` en `chat()` y `chat_stream()`.
- `db/repos`: las doce claves de generación viven en `settings` y se leen en cada llamada.
- `workers`: colapso, fichas y consolidación/compresión usan sus parámetros de `settings` y el
  modo JSON.
- `orchestrator/agent`: el chat usa temperatura, razonamiento y tokens de `settings`.
- `frontend`: nueva pestaña «Generación» en el diálogo de ajustes.

## Impact

- **Specs**: `llm/provider`, `llm/openrouter`, `db/repos`, `workers`, `orchestrator/agent`,
  `frontend` (deltas `ADDED`).
- **Código**: `src/llm/provider.rs` (tipos + `ChatRequest`); `src/llm/openrouter.rs` (cuerpos de
  `chat` y `chat_stream`); `src/llm/ollama.rs` (ignora ambos campos); un helper de lectura/parseo
  de los parámetros de generación desde `settings`; `src/workers/collapse.rs` y
  `src/workers/episodic_memory.rs`; `src/orchestrator/agent.rs` (el camino del chat).
- **Migración**: nueva migración que siembra las doce claves con sus defaults.
- **Frontend**: `types.ts`, `components/SettingsDialog.tsx` (+ test), reutilizando
  `updateSettings`; una pestaña «Generación» con cuatro bloques de tres campos.
- **Sin variables de entorno nuevas** para esto y **sin tocar** `docker-compose.prod.yml`.
