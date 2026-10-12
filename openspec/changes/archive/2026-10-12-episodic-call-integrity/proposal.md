# Proposal: Las llamadas episódicas no salen huecas ni en silencio

## Why

Tres defectos de forma en las llamadas LLM de la memoria episódica quedaron probados contra
OpenRouter con los prompts y las formas **reales de producción** (22 llamadas, cada caso
reproducido **3 veces**). Los tres comparten el mismo patrón: el código no ve el fallo, porque el
fallo es un **200 correcto con contenido inútil** o una **contradicción del prompt**, y la
estadística lo registra como éxito.

- **El consolidador manda un único mensaje `system`.** Con esa forma, `deepseek/deepseek-v4.1-flash`
  devuelve `completion_tokens = 1`, `content = null` y `finish_reason = "stop"`: **vacío y sin
  error** (3/3). Con un turno de usuario, el mismo prompt devuelve 966–991 tokens de JSON válido con
  `schema_version: 1` (3/3). Esto reproduce la avería real del 9 y 10 de octubre: **54 filas
  `kind='consolidator'` con `completion_tokens = 1`**. Como `extract_json_object("")` no encuentra
  objeto, devuelve `Err` y *«a failure here aborts the pass»*: dos días sin memoria persistente ni
  hechos.
- **El prompt sembrado del extractor se contradice.** Sus reglas dicen «si el lote no contiene
  ningún hecho, devuelve una **lista vacía**», mientras el formato de salida exige el objeto
  `{"events": [...]}`. `mistral` **obedece la prosa** y emite `[]` (3/3 sobre el lote real), que el
  parser rechaza como `NotJson`; es la causa del **35 % de los errores del timeline** en producción.
  El respaldo del código (`DEFAULT_TIMELINE_PROMPT_FALLBACK`) no tiene la contradicción: solo el
  **valor sembrado**.
- **Una validación fallida se registra como `success`.** La fila de `llm_requests` del consolidador
  registra el resultado **HTTP**, no la validación posterior: las 54 respuestas vacías salieron con
  `status='success'` y el fallo solo vivía en un `tracing::warn!`. Dos días a ciegas.

## What Changes

- **NUEVO — La petición del consolidador SHALL incluir un turno de usuario además del de sistema.**
  Un contenido vacío deja de ser posible *por construcción*; el modelo que exige un turno de usuario
  devuelve JSON en vez de `null`.
- **MODIFICADO — El prompt sembrado del extractor SHALL pedir el envoltorio `{"events": []}`** para
  el lote sin hechos, y SHALL NOT pedir una lista suelta. Se arregla el **valor sembrado** mediante
  una migración nueva, respetando la personalización del usuario (mismo idioma que
  `20261011000001_timeline_fact_integrity.sql`).
- **MODIFICADO — La fila de `llm_requests` del consolidador SHALL reflejar el resultado de la
  validación**: `status = 'error'` y el motivo en `error_message` cuando el contenido no valide. La
  avería deja de ser silenciosa.
- **MODIFICADO — Un `[]` suelto del extractor deja de ser «cero hechos» y SHALL tratarse como fallo
  de forma** (decisión resuelta del `design.md`, Alternativa 2): SHALL agotar los reintentos y, solo
  tras agotarlos, la pasada SHALL continuar sin hechos con `status = 'error'`; el cero hechos
  legítimo SHALL ser el envoltorio `{"events": []}`, que queda con `status = 'success'`. Se retira la
  tolerancia del array vacío que introdujo el change archivado `timeline-extractor-resilience`.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `workers`: un requisito **añadido** (la forma de la petición del consolidador) y tres
  **modificados** (el prompt sembrado del extractor, el registro de stats del consolidador y el
  best-effort de la extracción de hechos —el `[]` suelto pasa a ser fallo de forma—).

## Impact

- **Backend:** `src/workers/episodic_memory.rs` — `call_semantic_chat` (la forma de `messages`) y el
  punto donde se registran los stats de la consolidación.
- **Base de datos:** una **migración nueva** que reemplaza el valor sembrado de `timeline_prompt`
  solo si sigue siendo el texto anterior exacto.
- **Spec:** delta en `workers` (1 ADDED + 3 MODIFIED).
- **UI:** ninguna. El prompt sigue siendo editable desde la UI de ajustes; la migración no pisa una
  edición del usuario.

## Non-goals

- **No se cambia la configuración de modelos** (`SEMANTIC_MODEL`, `TIMELINE_MODEL`,
  `COLLAPSE_MODEL`, `MEMORY_MODEL`): con el turno de usuario, DeepSeek vuelve a ser viable. La
  elección de modelo pasa a ser una decisión de **coste**, no de viabilidad, y no forma parte de
  este change.
- **No se cambia el contrato best-effort de la Capa D**: la pasada sigue sin abortar y `is_indexed`
  se sigue escribiendo igual. Lo único que cambia es que un `[]` suelto **deja de contar como cero
  hechos** y pasa a agotar los reintentos como cualquier otro fallo de forma (ver la decisión
  resuelta del `design.md`).
- **No se toca `docker-compose.prod.yml`.**
- **No se toca el frontend** ni se añade ninguna vista.
- **No se rediseña el sistema de plantillas** (`{{ ESTADO_ACTUAL }}` / `{{ BLOQUE_DE_MENSAJES }}`)
  sin justificarlo; el arreglo de la petición se limita a la forma de `messages`.
