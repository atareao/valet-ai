# PLAN-005 — Activity Timeline (línea temporal de hechos)

**Proyecto:** Valet (Rust/Axum + React + SQLite)
**Estado:** ✅ Completado — change `activity-timeline` aprobado, implementado (TDD) y archivado; PR en curso.
**Origen:** especificación técnica «Activity Timeline & Life Journal», aportada por el usuario el 2026-10-10
**Fecha:** 2026-10-10
**Metodología:** OpenSpec (SDD) + TDD (Red-Green-Refactor)

---

## Contexto (lo que hay hoy)

- **Stack:** Rust + Axum 0.8 + **SQLite** (sqlx) + `sqlite-vec`; frontend React/Vite. El despliegue real es un contenedor (`valet`) tras Traefik, sin puertos publicados.
- **Tres memorias:** `messages` (el bruto), las fichas episódicas vectorizadas (`memory` + `vec_memory`, recuperación por similitud) y el estado persistente consolidado. Ninguna responde a «¿qué hice el martes?»: la ventana de una ficha (`first_message_at` / `last_message_at`) no se puede filtrar ni ordenar por día.
- **`EpisodicMemoryWorker`** (`src/workers/episodic_memory.rs`): procesa lotes de mensajes con `is_indexed = 0`. El trigger ya existe (`MEMORY_BATCH_TOKENS=2000`, `MEMORY_INACTIVITY_MINUTES=30`, poll cada 30 min) y escribe **dos capas en una pasada y con una sola marca** (ficha = Capa B, estado = Capa C). `is_indexed` **es su cursor**: lo pone a 1 en la misma transacción.
- **Diecisiete herramientas registradas** (`build_tool_registry` en `src/lib.rs`) en un **catálogo cerrado de siete skills** (`src/orchestrator/skills.rs`): `agenda`, `pendientes`, `recuerdos`, `entorno`, `web`, `widgets` y `running`. Conjunto core no enrutable: `get_current_time` y `get_current_location`. Un test de integridad obliga a que toda herramienta registrada pertenezca al core o a alguna skill.
- **Patrón de herramientas:** `calendar`, `tasks`, `notes` y `reminders` son **fachadas con `operation`**; las cuatro `strava_*` son herramientas sueltas. Permisos: `NoConfirm`, `Notify` y `ExplicitApproval`.
- **`llm_requests.kind`** distingue los cinco orígenes de llamada (`chat`, `router`, `archivist`, `consolidator`, `collapse`) con un `CHECK` en línea; `CallKind` (`src/models/stats.rs`) es su espejo en Rust.
- **Roles de generación** (`src/generation.rs`): `GenerationRole` con cuatro roles (`GENERATION_<ROL>_TEMPERATURE`, `_REASONING` y `_MAX_TOKENS`); una clave ausente cae al default del rol.
- **Prompts de worker** sembrados en `settings`: `archivist_prompt`, `consolidator_prompt` y `collapse_prompt`, con el idioma de upsert idempotente de `20260929000001_prompts.sql`.

---

## Tema 1 — Activity Timeline

**Estado:** ✅ Completado. Las 27 tareas de código/documentación están en verde (`1120 passed / 0 failed`, clippy y fmt limpios) y solo queda la comprobación en producción (tarea 5.4). Change archivado con `openspec archive activity-timeline`.

### Objetivo

Que Valet sepa qué hizo el usuario y cuándo. Hoy «¿qué hice el martes?» no tiene respuesta exacta: la memoria episódica recupera por similitud y la ventana de una ficha no se filtra por día. Este tema añade un registro **cronológico y determinista** de hechos atómicos —una tabla `timeline_events`, alimentada automáticamente desde la conversación— y tres herramientas para consultarlo, anotarlo y borrarlo.

### Lo decidido (2026-10-10)

| # | Decisión | Elección | Por qué |
|:--|:--|:--|:--|
| **D1** | Dónde vive el extractor | **Tercera extracción dentro de la pasada del `EpisodicMemoryWorker`** (Capa D), con el mismo lote y la misma marca | `messages.is_indexed` **es el cursor del worker**: dos consumidores sobre la misma bandera se pisan. Y el trigger que pedía el spec (2.000 tokens / 30 min de inactividad) **ya existía**: era el de este worker. Además, el worker ya escribe dos capas en una pasada con una sola marca; esta es la tercera |
| **D2** | Fallo del extractor | **Best-effort: degrada y no aborta** — la ficha, el estado y la marca se escriben igual; se registra el diagnóstico y la estadística queda con `status = error` | Un extractor inservible no puede tirar abajo la memoria episódica. Es el mismo criterio que el change aparcado `llm-worker-resilience` (su decisión D1) |
| **D3** | Forma de las herramientas | **Tres herramientas sueltas**: `timeline_get_events`, `timeline_add_event` y `timeline_delete_event` | Elección del usuario. Las cuatro `strava_*` ya siguen ese patrón |
| **D4** | Skill | **Nueva skill `timeline`** (la octava del catálogo), con su fragmento de prompt y su clave de habilitación | «¿Qué hice ayer?» es un dominio propio; sin skill nueva, las herramientas habrían caído dentro de `recuerdos` (notas + búsqueda), mezclando dos cosas distintas |
| **D5** | Exportador de diario | **No se hace** | Decisión del usuario. Además, el despliegue es un contenedor y no ve el `$HOME` del host: `~/Diario/*.md` no existe como tal |
| **D6** | Sitio del plan | **`plans/PLAN-005.md`** (este fichero) | Es una feature completa —migración, worker, herramientas y skill—; el `PLAN-004` sigue aparcado y no se toca |

### Correcciones al spec de origen

El spec de partida se escribió contra **PostgreSQL y un Valet imaginario**. Estas son sus ocho divergencias con el repo real, todas ya resueltas en el change:

| El spec decía | Se ha resuelto como |
|:--|:--|
| PostgreSQL: `CREATE TYPE … ENUM`, `gen_random_uuid()`, `TIMESTAMPTZ`, `DO $$ … $$` | **SQLite**: `id TEXT`, `timestamp TEXT` en ISO 8601, `category TEXT … CHECK(…)` y `created_at TEXT DEFAULT (datetime('now'))` |
| Tool `time_get_current` | `get_current_time`, que ya pertenece al conjunto core (siempre expuesta) |
| Trigger: 2.000 tokens / 30 min | **Ya existía**: `MEMORY_BATCH_TOKENS`, `MEMORY_INACTIVITY_MINUTES` y poll cada 30 min |
| El worker lee `messages.is_indexed = FALSE` | Esa bandera **la posee** el `EpisodicMemoryWorker` (véase D1) |
| Skill como fichero `skills/activity_timeline.md` | El catálogo es **cerrado y vive en Rust**; el prompt se siembra en `settings` |
| `timestamp` desde «FECHA Y HORA ACTUAL DEL SISTEMA» | El de **cada mensaje** (`created_at`); usar la hora actual fecharía mal todo lo pasado |
| Exportar a `~/Diario/YYYY-MM-DD.md` | **Fuera de alcance** (D5) |
| `timeline_events` sin `profile_id` | Se mantiene **sin** `profile_id`, como su origen (`messages` y `memory`), y no como `notes` o `tasks` |

### Alcance

**Dentro**

- La tabla `timeline_events` con sus índices y su `CHECK` de categoría.
- La Capa D en el `EpisodicMemoryWorker`: extracción, validación y persistencia dentro de la transacción existente.
- `CallKind::Timeline` (`kind = 'timeline'`) y la ampliación del `CHECK` de `llm_requests`, que exige reconstruir la tabla **sin perder filas**.
- `GenerationRole::Timeline` y `TIMELINE_MODEL`, que cae a `MEMORY_MODEL`.
- Las tres herramientas del timeline y su registro.
- La skill `timeline` en el catálogo cerrado, con sus dos claves sembradas en `settings`.

**Fuera (no-objetivos)**

- **El exportador de diario a Markdown**: no se hace (D5).
- Ningún cambio en la memoria episódica ni en el consolidador.
- Nada de interfaz: el timeline se consulta hablando. No hay widget nuevo.
- No hay ajuste para desactivar la extracción: el dominio se apaga deshabilitando la skill, que solo afecta al enrutado.

### Mapa técnico

| Zona | Ficheros |
|:--|:--|
| Persistencia | `migrations/<fecha>_timeline_events.sql`, `migrations/<fecha>_timeline_prompts.sql` y `src/db/repos/timeline.rs` |
| Extractores | `src/workers/episodic_memory.rs` (la Capa D), `src/generation.rs`, `src/models/stats.rs` y `src/config.rs` |
| Herramientas | `src/tools/timeline.rs`, `src/tools/mod.rs` y `src/lib.rs` |
| Catálogo de skills | `src/orchestrator/skills.rs` |
| Plantillas | `.env.example` y `.env.j2` |

### Tareas (TDD)

El checklist completo (RED → GREEN → REFACTOR → plantillas → VERIFY) está en `openspec/changes/activity-timeline/tasks.md`, con 28 tareas. Resumen de las fases:

- **F0** — aprobación explícita del change (bloqueante).
- **F1 (RED)** — migraciones, parser, normalización, la pasada (incluida la degradación), el prompt, el contrato y los permisos de las tres herramientas, el catálogo y el modelo.
- **F2 (GREEN)** — las dos migraciones, el repo, `CallKind` y `GenerationRole`, la Capa D, las herramientas, la skill y `TIMELINE_MODEL`.
- **F3 (REFACTOR)** — `fmt`, `clippy --all-targets -- -D warnings`, catálogo y registry revisados.
- **F4** — `TIMELINE_MODEL` documentado en las plantillas.
- **F5 (VERIFY)** — las suites, `openspec validate --strict`, la review de `@rust-reviewer`, `openspec archive` y la comprobación en producción.

### Definición de Hecho (DoD)

- [ ] El change `activity-timeline` está **aprobado** por el usuario.
- [ ] Un lote de mensajes produce hechos en `timeline_events`, con la fecha de su mensaje de origen y su `source_message_id` (test).
- [ ] Un extractor que devuelve vacío o JSON inválido **no aborta la pasada**: la ficha, el estado y la marca se escriben igual (test).
- [ ] La migración de `llm_requests` conserva todas las filas previas y acepta `kind = 'timeline'` (test).
- [ ] `timeline_get_events` filtra por rango y por categoría con límite, `timeline_add_event` usa la hora actual cuando se omite el `timestamp`, y `timeline_delete_event` exige aprobación explícita (tests).
- [ ] El catálogo tiene **ocho** skills y **veinte** herramientas, y el test de integridad pasa.
- [ ] `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` y `openspec validate activity-timeline --strict` en verde.
- [ ] Review de `@rust-reviewer` con los hallazgos aplicados.
- [ ] `openspec archive activity-timeline` y este plan cerrado.
- [ ] En producción: hay filas de `llm_requests` con `kind = 'timeline'` y `timeline_events` crece con los días.

### Riesgos / notas

- **La reconstrucción de `llm_requests` es el punto delicado.** SQLite no permite alterar un `CHECK` en línea, así que hay que recrear la tabla y copiar las filas. La estadística histórica es valiosa —es la que sostiene el panel y los diagnósticos— y una migración mal hecha la pierde. Los tests de la F1 lo cubren explícitamente.
- **La cobertura del catálogo es un test, no un acuerdo.** Registrar las tres herramientas y olvidarlas en el catálogo hace fallar la suite: es la red que impide que el timeline quede fuera del enrutado sin que nadie se entere.
- **El change aparcado `llm-worker-resilience` sigue activo** y este tema añade una **tercera** llamada con riesgo de respuesta vacía. Por eso la Capa D nace con su propia guarda (D2) y no depende de aquel change; si se retoma, los dos se refuerzan.
- **Solape con `PLAN-004-PENDING.md` §2 (Bitácora).** Aquel roadmap diseña una bitácora **manual** con ánimo y reflexión (`journal_entries`, con los workers `JournalPrompt` y `JournalDigest`). Este tema es la mitad **automática**: los hechos extraídos de la conversación. No se toca `journal_entries` ni se implementa nada de aquella sección. Si algún día se retoma, los hechos del timeline son una fuente natural de contexto para la pregunta diaria, pero eso no está decidido.

---

## Temas pendientes

Tema 1 **cerrado** (change `activity-timeline` implementado y archivado). Sin más temas abiertos en este plan.
