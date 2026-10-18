# Proposal: La bitácora autónoma

## Why

Hoy la bitácora **hay que pedírsela**. El change `journal-entries` (archivado el 2026-10-18, PR #215)
dejó el registro completo —tabla `journal_entries` con una entrada por perfil y día, `JournalRepo`, la
herramienta `journal` y su sección en la app—, y desde entonces el usuario puede escribir, corregir,
listar y borrar sus entradas; lo que **no** puede es que Valet las escriba. Una bitácora que solo
existe cuando alguien se acuerda de dictarla no es una bitácora: es un formulario.

`plans/PLAN-007.md` decidió que la escriba el asistente **por su cuenta**, juzgando él mismo cuándo
hay algo que merezca quedar y qué contar. Este change es esa mitad: la autonomía. La tabla, el
repositorio y la herramienta ya están; lo que falta es quien los use.

El repositorio ya tiene los materiales que hacen falta para hacerlo sin inventar nada: el patrón de
worker en segundo plano (`EpisodicMemoryWorker`), la resiliencia ante respuestas LLM inservibles
(`llm-worker-resilience`, archivado el 2026-10-18: degradar en vez de abortar, presupuesto doblado al
truncar, fila honesta en `llm_requests`), el timeline de hechos del día (`PLAN-005`) y la siembra
editable de prompts por migración (`episodic-call-integrity`).

## What Changes

- **Un worker propio, `JournalWorker`**, con su ciclo y su tarea, que en cada vuelta juzga **el día
  cerrado más antiguo que aún no se haya juzgado**: un parón de varios días se recupera día a día y
  ningún día se procesa dos veces.
- **El «día» es el del usuario.** Se calcula con `timezone` (el ajuste que guarda el navegador) y con
  una **hora de corte** configurable (`journal_hour`, por defecto 4): a las 04:05 del 19 de octubre se
  juzga el 18. Sin zona válida se cae a UTC. El cálculo vive en **funciones puras** y se prueba a
  conciencia, cambios de hora incluidos.
- **Qué mira**: los mensajes de ese día (la textura), los hechos del timeline de ese día (el
  esqueleto) y **la entrada anterior** (para seguir el hilo y no repetirse), con un tope de tokens.
  **Sin material no hay llamada al modelo**: un día sin mensajes ni hechos se marca como juzgado sin
  gastar un céntimo.
- **Qué escribe**: una entrada de `journal_entries` por `upsert` (relato y, opcionalmente, ánimo,
  logros, dificultades y etiquetas). Y **puede no escribir nada**: «esa jornada no tiene nada que
  contar» es una respuesta válida y definitiva.
- **El progreso vive en un ajuste** (`journal_last_judged_day`, una fecha ISO). Un día juzgado no se
  vuelve a juzgar nunca, ni siquiera cuando la respuesta fue «nada que contar». En su primer arranque
  el worker **no** recorre la historia: empieza por el último día cerrado.
- **El relato manda sobre los hechos.** El prompt vive en los ajustes (`journal_prompt`), lo siembra
  una migración **solo si nadie lo ha editado**, y pide lectura —cómo fue, qué costó, qué ilusiona—,
  no un recuento: qué pasó ya lo escribe el timeline.
- **Nace encendida, con ajuste para apagarla** (`journal_enabled`), hora (`journal_hour`) y modelo por
  variable de entorno propia (`JOURNAL_MODEL`, que cae a `MEMORY_MODEL`), sin sembrar modelos que
  razonan en las plantillas.
- **Cuando el modelo falla**: se reintenta una vez —con presupuesto doblado si el corte fue por
  techo—, y si vuelve a fallar se registra la fila honesta en `llm_requests` (`kind = 'journal'`,
  `status = 'error'`, motivo de finalización, uso y preview) y **el día se marca como juzgado**. El
  fallo se ve en las estadísticas y el diario pierde ese día; la bitácora nunca entra en el bucle
  infinito que costó dos días de memoria parada. Un fallo del worker de la bitácora no puede afectar a
  ningún otro worker.
- **`llm_requests.kind` admite `'journal'`**: como SQLite no permite alterar un `CHECK` en línea, una
  migración reconstruye la tabla preservando todas las filas, y el origen entra en las listas cerradas
  que lo hacen visible (agregación de procesos de fondo y su etiqueta en la app) — sin él, la fila
  honesta que pide la autonomía se insertaría, pero nadie la vería.

## Capabilities

### New Capabilities

Ninguna. La bitácora ya tiene sus capacidades; este change no estrena ninguna.

### Modified Capabilities

- `workers`: **el grueso del change**, todo añadido (8 requisitos nuevos): el worker y su ciclo, el
  cálculo del día (funciones puras), los ajustes que lo gobiernan, el material y la escritura, el
  prompt sembrado del relato, el manejo del fallo, el registro de sus llamadas y su arranque en el
  `WorkerPool`. Nada de lo existente se modifica.
- `db/schema`: **1 modificado** (el `CHECK` de `llm_requests.kind` admite además `'journal'`,
  reconstruyendo la tabla sin perder filas) y **1 añadido** (la migración que siembra
  `journal_prompt`, `journal_enabled` y `journal_hour` sin pisar una personalización).
- `db/repos`: **2 modificados** — la agregación de procesos de fondo (`journal` entra como origen
  canónico, o sus filas se descartarían en silencio) y la marca de llamadas sospechosas (`journal`
  es un origen estructurado más).
- `frontend`: **1 modificado** — la tarjeta «Procesos de fondo» gana la etiqueta legible del origen
  `journal`, que hoy caería al `kind` crudo.

## Impact

- **Backend:** `src/workers/journal.rs` (worker nuevo, con su función pura del día), `src/workers/mod.rs`,
  `src/workers/pool.rs` (arranque y parada), `src/config.rs` (`journal_model` desde `JOURNAL_MODEL`) y
  `src/models/stats.rs` (`CallKind::Journal`). El rol de generación se **reutiliza** (ver `design.md`,
  D11): `src/generation.rs` no se toca.
- **Base de datos:** una **migración nueva** que (a) amplía el `CHECK` de `llm_requests.kind` a
  `'journal'` y (b) siembra `journal_prompt`, `journal_enabled` y `journal_hour`. No se toca ninguna
  columna de `journal_entries`.
- **Frontend:** `frontend/src/components/stats/BackgroundCard.tsx` (una etiqueta) y su test colocado.
- **Plantillas:** `.env.j2` y `.env.example` ganan `JOURNAL_MODEL` (sin modelos que razonan).
- **Spec:** deltas en `workers`, `db/schema`, `db/repos` y `frontend`.
- **Sin dependencias nuevas.** Sin rutas nuevas, sin cambios en la herramienta `journal` ni en la
  skill `recuerdos`, sin tocar el esquema de `journal_entries` y sin tocar `docker-compose.prod.yml`.

## Non-goals

Lo siguiente se descarta **por diseño** (`plans/PLAN-007.md`, «Qué se descarta») y NO entra aquí:

- **El resumen semanal** (`JournalDigest`) y la herramienta de «insights».
- **Los avisos por push** al escribir una entrada y **el clima** (`weather_context`): si el tiempo
  importa, saldrá en el relato.
- **Vistas nuevas en la app**: la sección de Bitácora ya existe.

Y además:

- **No se toca la herramienta `journal` ni la skill `recuerdos`**: el worker escribe por el
  repositorio, no por la herramienta del agente.
- **No se toca el timeline ni la memoria**: el worker **lee** los hechos del día y los mensajes; no
  reescribe nada suyo.
- **No se estrena un rol de generación nuevo**: el modelo del diario es configuración por entorno y
  sus parámetros salen del rol mecánico existente (ver `design.md`). Un rol propio, con su fila en la
  pestaña «Generación», sería otro change.
- **No se añade una sección de ajustes de la bitácora a la interfaz**: `journal_prompt`,
  `journal_enabled` y `journal_hour` son claves de `settings` y se leen y se escriben por
  `GET`/`PUT /api/settings`, como el resto; la pestaña propia sería otro change.
- **No se migra ni se reescribe ninguna entrada existente**: la bitácora escrita a mano manda.
- **No se toca `docker-compose.prod.yml`** ni la deriva de las plantillas de despliegue.
