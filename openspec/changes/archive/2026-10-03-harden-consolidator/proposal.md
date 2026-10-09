# Change: Endurecer el consolidador de memoria persistente (Capas B/C)

## Why

El worker episódico aborta la pasada con `consolidator returned an invalid state: consolidator
returned no JSON object`. Diagnóstico con evidencia de producción:

- Settings reales: `GENERATION_SEMANTIC_REASONING = low`, `GENERATION_SEMANTIC_MAX_TOKENS = 2048`,
  modelo `deepseek/deepseek-v4.1-flash` (modelo de razonamiento).
- `llm_requests` de la llamada de las 13:35:48: `completion_tokens = 2048`, `reasoning_tokens = 2048`.
- Log del provider: `content_len=0`. Los 2048 tokens del `max_tokens` se consumieron **íntegros en
  razonamiento**, el `content` quedó vacío y `extract_json_object("")` devolvió `None`.
- Es **intermitente**: otra llamada idéntica gastó 111 en razonamiento y funcionó.

La spec asumía que 2048 "deja holgura frente a los tokens de razonamiento". Con un modelo de
razonamiento esa holgura no existe: el razonamiento puede ocupar todo el presupuesto. Además, la
calidad del prompt sembrado es mejorable: mezcla datos de perfil con `system_rules`, permite
secciones vacías e infiere datos no afirmados.

Banco de pruebas empírico (48 llamadas por variante, `reasoning: off`):

| Prompt | Esquema válido | Extracción | Verboso (tokens) |
|---|---|---|---|
| Sembrado actual | 30/30 | reglas 3/5 | 64–67 |
| Taxonomía cruda | 26/30 | contradicción 3/5 | 123–174 |
| **V8 refinado** | **48/48** | **8/8 en todos** | **118–165** |

Y con `reasoning: low` el 100% de las llamadas de la muestra devolvió `content_len=0`.

## What Changes

- **El consolidador no razona por defecto.** `GENERATION_SEMANTIC_REASONING` pasa de `low` a `off`,
  alineado con los roles de colapso y fichas ("no pagar razonamiento en tareas mecánicas"). El
  default en código y una migración corrigen el valor heredado `low` (respetando valores distintos).
- **Prompt del consolidador refinado (V8).** Estructura `user_profile` por taxonomía
  (`identity`, `preferences_and_tastes.{communication_style,technology_and_tools,lifestyle_and_leisure,dislikes_and_dealbreakers}`,
  `lifestyle_and_routines`, `productivity_and_workflow`, `interests_and_knowledge`,
  `relationships_and_entities`), prohíbe secciones vacías e inferencias, y separa `system_rules`
  (solo instrucciones explícitas al asistente) de los datos de perfil. Se siembra por migración y
  actualiza la plantilla de respaldo en código.
- **Reintento de seguridad.** Si la consolidación inicial devuelve contenido vacío, no-JSON o
  esquema inválido, el worker reintenta **una vez** antes de abortar la pasada.
- **Diagnosabilidad.** El error incluye `content_len` y un preview corto del contenido, para no
  depender del log del provider.
- **Presupuesto de memoria a 800.** `PERSISTENT_MEMORY_BUDGET_TOKENS` pasa de 500 a 800 (techo
  1600) para dar holgura a los estados más ricos que produce el prompt V8, sin comprimir tan pronto.

## Capabilities

### Modified Capabilities
- `db/repos`: el valor inicial de `GENERATION_SEMANTIC_REASONING` pasa a `off` y una migración
  corrige el `low` heredado.
- `workers`: la consolidación no razona por defecto; reintento único antes de abortar; diagnóstico
  del contenido en el error.
- `persistent-memory`: el prompt sembrado estructura el perfil por taxonomía y evita redundancia;
  el presupuesto por defecto pasa a 800.
- `persistent-memory-ui`: la lectura HTTP refleja el presupuesto por defecto de 800.

## Impact

- Backend: `src/generation.rs` (default del rol Semantic), `src/workers/episodic_memory.rs`
  (plantilla de respaldo, reintento y diagnóstico), `src/persistent_memory.rs` (default del
  presupuesto a 800), nueva migración `migrations/<fecha>_consolidator_reliability.sql`.
- Tests: default `off`, migración `low→off`, prompt sembrado (taxonomía y placeholders), reintento
  (vacío→válido) y diagnóstico (`content_len`).
- Sin cambios de esquema de tablas, ni de contrato LLM, ni de UI. `docker-compose.prod.yml` intacto.
- No se editan migraciones existentes (sqlx valida su checksum); se añade una nueva.

### Fuera de alcance

- Reintentos con backoff o circuit breaker.
- Migración del estado persistente existente a la nueva taxonomía (aceptamos perderlo).
