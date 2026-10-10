# Change: Activity Timeline — línea temporal de hechos y consulta por fecha

## Why

La memoria episódica guarda fichas sintéticas y vectorizadas: excelentes para «¿qué sé de X?» por
similitud, inservibles para «¿qué hice el martes?», que es una pregunta **exacta de rango**. Valet
hoy no tiene ninguna estructura cronológica: los hechos con fecha viven diluidos dentro de fichas
cuya ventana (`first_message_at` / `last_message_at`) no se puede filtrar ni ordenar por día.

Esta feature añade un registro **determinista** de hechos atómicos con su fecha: una tabla
`timeline_events` alimentada en segundo plano desde la propia conversación, y tres herramientas para
consultarla, anotarla y borrarla. No sustituye a la memoria episódica: la complementa por el lado que
a ella se le escapa —el eje del tiempo—.

## What Changes

- **Nueva tabla `timeline_events`** (SQLite): `timestamp` en ISO 8601 UTC, `category` con un conjunto
  cerrado garantizado por `CHECK`, `fact` atómico en pasado, `source_message_id` (clave ajena a
  `messages` con `ON DELETE SET NULL`) y `created_at`; con índices por fecha, por categoría y por
  mensaje de origen.
- **Nueva Capa D en el `EpisodicMemoryWorker`**: una **tercera extracción** en la misma pasada, con
  el mismo lote y la misma marca, que pide al LLM los hechos del lote y los persiste dentro de la
  transacción existente. Es **best-effort**: si el extractor devuelve algo inservible se registra el
  diagnóstico y **la pasada sigue** (ficha, estado y marca intactos). El extractor recibe el
  `created_at` de cada mensaje y fecha cada hecho con él, nunca con la hora del sistema.
- **Tres herramientas nuevas**: `timeline_get_events`, `timeline_add_event` y
  `timeline_delete_event`. El catálogo de herramientas pasa de diecisiete a **veinte**.
- **Nueva skill `timeline`** en el catálogo cerrado, que pasa de siete a **ocho** skills, con su
  fragmento `SKILL_TIMELINE_PROMPT` y su clave de habilitación `ROUTER_SKILL_TIMELINE_ENABLED`.
- **`llm_requests.kind` admite `'timeline'`**: como SQLite no permite alterar un `CHECK` en línea, la
  migración reconstruye la tabla **preservando todas las filas**, y `CallKind` gana la variante.
- **Nuevo modelo configurable** `TIMELINE_MODEL`, que cae a `MEMORY_MODEL`, con su propio rol de
  generación `GENERATION_TIMELINE_*`.

## Impact

### Specs nuevas

- `tools/timeline`: contrato de las tres herramientas (rangos ISO 8601, categorías cerradas, límite y
  permisos).

### Specs modificadas

- `workers`: la Capa D dentro de la pasada del `EpisodicMemoryWorker`, su carácter best-effort, la
  validación de cada hecho y el extractor como origen de estadísticas `kind='timeline'`.
- `orchestrator/skill-router`: el catálogo pasa de siete a ocho skills y de diecisiete a veinte
  herramientas.
- `tools/registry`: el registry de producción registra las tres herramientas nuevas.
- `db/schema`: la migración de `timeline_events` y la ampliación de los valores de
  `llm_requests.kind`.

### Código

- `migrations/` (dos ficheros nuevos), `src/db/repos/timeline.rs`, `src/tools/timeline.rs`,
  `src/tools/mod.rs`, `src/lib.rs`, `src/config.rs`, `src/generation.rs`, `src/models/stats.rs`,
  `src/orchestrator/skills.rs` y `src/workers/episodic_memory.rs`.

### Datos

- Nueva tabla `timeline_events`. Reconstrucción de `llm_requests` sin pérdida de filas.

### No-objetivos

- **No hay exportador de diario**: no se escriben ficheros Markdown de bitácora.
- No se toca la memoria episódica ni el consolidador: el timeline es otra cosa.
- No hay interfaz: el timeline se consulta hablando.
- No hay widget nuevo.
