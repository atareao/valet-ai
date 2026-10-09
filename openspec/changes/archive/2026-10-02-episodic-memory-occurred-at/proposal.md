# Proposal: Registrar y usar la fecha en que ocurrieron los hechos de la memoria episódica

## Why

La memoria episódica guarda en cada ficha un `created_at` que es **cuándo se escribió la ficha**, no cuándo ocurrió lo que resume. Eso hace dos daños.

**1. En el prompt.** Cada ficha se inyecta como `[{created_at}] {content}`. Al modelo se le dice la fecha de escritura, que puede no tener nada que ver con la de los hechos. Medido en la base real de producción:

| ficha | `created_at` (lo que ve el modelo) | hechos reales |
|---|---|---|
| edda7a1e | 2026-10-02 06:18 | 2026-09-29 17:13 → 2026-09-30 17:37 |
| c3a4de3d | 2026-10-02 06:48 | 2026-10-01 04:20 → 2026-10-01 17:30 |

Y es redundante: el contenido de la ficha ya abre con una línea `FECHA/CONTEXTO:` que el propio LLM escribe, pensada para leerse. La fecha en corchetes duplica, peor, algo que el texto ya dice mejor.

**2. En el decaimiento.** `final = similitud × exp(-ln2 × días / MEMORY_HALF_LIFE_DAYS)` mide `días` desde `created_at`. En régimen normal el worker archiva media hora después de la conversación, así que es un proxy aceptable. Se rompe en dos casos reales:

- **Una reconstrucción**: todas las fichas nacen con la fecha de hoy, como acaba de pasar. Las dos fichas actuales pesan igual y el decaimiento no ordena nada.
- **Una caída larga**: al reanudar, todo un atraso entra con fecha de hoy.

El dato exacto ya está a mano: `metadata.primary_message_ids` apunta a los mensajes de origen, y sus `created_at` dan el intervalo real. Hoy mismo se puede comprobar con un JOIN. El esquema de `memory` es `id, content, tokens_count, created_at, metadata` — **`metadata` es TEXT con JSON**, y ya guarda `source`, `primary_message_ids` y `date_context`.

## What Changes

- **D1 — La ficha SHALL registrar cuándo ocurrió lo que resume.** Al persistir, el `EpisodicMemoryWorker` SHALL escribir en `metadata` dos claves nuevas, `first_message_at` y `last_message_at`, tomadas del `created_at` del mensaje de origen más antiguo y del más reciente de los que componen la ficha. **Sin cambio de esquema**: `metadata` ya es JSON y ya lleva tres claves.
- **D2 — El decaimiento SHALL medirse contra `last_message_at`, no contra `created_at`.** Se elige el extremo **más reciente** porque una ficha es tan actual como su contenido más nuevo, y anclar al final es monótono. Cuando la clave no exista (fichas escritas antes de este cambio), SHALL caer a `created_at`, para no romper nada. `first_message_at` se guarda además para el registro y para poder cambiar el criterio sin reconstruir.
- **D3 — La ficha inyectada SHALL NOT llevar ninguna fecha.** `format_memory` pasa a devolver el contenido tal cual. Justificación: la cronología ya la da el `FECHA/CONTEXTO` del texto, que está escrito para que lo lea un modelo; la fecha máquina sirve para decisiones de máquina (el ranking), no para el prompt, y después de una reconstrucción miente.
- **D4 — Sin backfill.** Las fichas ya escritas se quedan como están y usan el fallback de D2. Con `MEMORY_HALF_LIFE_DAYS = 90`, la diferencia entre medir contra una fecha u otra en las dos fichas actuales es de un 1,2% y un 0,4%: inapreciable, y una reconstrucción las regeneraría igualmente.
- **D5 — `metadata.date_context` se queda como está.** Es el texto de fecha que escribe el LLM. **Ningún código lo lee** (solo se escribe y aparece en tests): se guarda para inspección humana. No se toca.

La fórmula del decaimiento y la regla de que **el decaimiento ordena, no excluye** (el umbral se aplica a la similitud y nunca al resultado final) se conservan intactas; lo único que cambia es de dónde salen los `días`.

## Capabilities

Se eligen las capabilities existentes cuyo alcance encaja con cada delta; no se crea ninguna capability nueva.

### New Capabilities

<!-- Ninguna -->

### Modified Capabilities

- `orchestrator`: el decaimiento temporal mide la antigüedad desde `metadata.last_message_at` con fallback a `created_at`, y la ficha formateada deja de llevar fecha.
- `orchestrator/agent`: el bloque de memoria deja de anclar cada ficha con su fecha; solo conserva el orden por relevancia final descendente.
- `workers`: el `EpisodicMemoryWorker` escribe `first_message_at` y `last_message_at` en la `metadata` de la ficha.

## Observaciones (no forman parte de este cambio)

- **D5, inobservancia de `metadata.date_context`.** Como se ha dicho, ningún código lee `date_context`: solo se escribe (en el worker) y aparece en tests. Se mantiene tal cual por si conviene inspeccionarlo a mano. Retirarlo o aprovecharlo sería otro cambio.
- **El bucle del worker consume y descarta el tick inmediato.** El comentario del código dice `// Tick immediately on start`, pero la llamada `interval.tick().await` que precede al `loop` **consume y descarta** el tick que `tokio::time::interval` dispara en el instante cero. El bucle real no vuelve a evaluar hasta pasado un periodo completo, así que **la primera evaluación espera un intervalo entero** y no ocurre al arrancar. Medido en producción: el contenedor arrancó a las **05:47:56 UTC** y la primera ficha se persistió a las **06:18:03 UTC**, treinta minutos después (un `MEMORY_POLL_INTERVAL_MINUTES` completo, default 30); la segunda, otro periodo después, a las **06:48:04 UTC**. Es un detalle real del arranque del worker, pero **NO forma parte de este cambio**; se anota solo como contexto.
- **El delta de `workers` prescribe el resultado, no el mecanismo.** El requisito exige las claves y sus valores (el `created_at` del mensaje de origen más antiguo y del más reciente), no cómo obtenerlos. Así la implementación puede derivarlos del lote en memoria que ya compone la ficha, sin atarla a una consulta a `messages` que no hace falta.
- **Deriva preexistente en `openspec/specs/orchestrator/spec.md` (corregida aquí).** El requisito «El decaimiento temporal SHALL calcularse en Rust y ordenar los resultados» atribuye el cálculo y el orden a «el constructor», pero eso nunca fue cierto: el cálculo (`final = similitud × exp(-ln2 × días / MEMORY_HALF_LIFE_DAYS)`) y el orden por `final` descendente los hace `MemoryRepo::search_by_vector`; el constructor solo aplica después el presupuesto de tokens. Como este cambio modifica ese mismo requisito de todos modos, el delta corrige también esa atribución: el sujeto del cálculo y del orden pasa a ser `MemoryRepo::search_by_vector`, y se deja dicho que el presupuesto de tokens lo aplica el constructor. Es una **corrección de deriva preexistente, no un cambio de comportamiento**.

## Impact

- **Backend Rust**: `src/workers/episodic_memory.rs` (derivar `first_message_at`/`last_message_at` del lote de mensajes de origen que compone la ficha y escribirlos en `metadata` al persistir); `src/db/repos/memory.rs` (los `días` del decaimiento pasan a leerse de `metadata.last_message_at`, con fallback a `created_at`: aquí vive el cálculo y el orden por `final`); `src/orchestrator/context_builder.rs` (solo `format_memory` sin fecha; aquí NO se toca el decaimiento).
- **Esquema de datos**: sin migración. `metadata` sigue siendo TEXT con JSON; solo se añaden dos claves.
- **Prompt / LLM**: cambia el texto inyectado por la memoria episódica (cada ficha pierde su prefijo de fecha). El orden de las fichas no cambia.
- **Datos**: sin backfill. Las fichas anteriores a este cambio usan el fallback por `created_at`.
- **Tests**: `src/workers/episodic_memory.rs`, `src/db/repos/memory.rs` para los del decaimiento y `src/orchestrator/context_builder.rs` para los del formato; se actualizan las aserciones que fijaban el formato `[{fecha}] {content}` y las que medían el decaimiento contra `created_at`. Cualquier otra aserción que se rompa es una regresión, no una actualización.
- **API**: sin cambios en la API pública HTTP.
