# Proposal: La bitácora como registro

## Why

Hoy lo que Valet sabe de un día del usuario está repartido: el **timeline** guarda hechos
automáticos y sueltos, y las notas (`notes`) son textos sin fecha ni estructura. No hay ningún sitio
donde vivir la **lectura de cómo fue el día** —el relato, el ánimo, lo que costó y lo que ilusionó—,
y por eso el asistente no puede ni recordarla ni escribirla.

Este change es la **mitad registrable** de `plans/PLAN-007.md` (decisión D9: tabla propia; D7: se lee
en la app y por el agente): la bitácora como almacén y como herramienta. Sobre él se montará la
autonomía (`journal-worker`), que necesita la tabla, el repositorio y la herramienta para poder
escribir el diario por su cuenta. Al terminar este change el usuario escribe, corrige, lista y borra
sus entradas; **Valet todavía no escribe solo**.

## What Changes

- **AÑADIDO — Migración `journal_entries`.** Tabla nueva con una fila por perfil y día
  (`UNIQUE(profile_id, date)`), fecha ISO `YYYY-MM-DD` en la zona del usuario, ánimo opcional de un
  conjunto cerrado, relato obligatorio y tres campos JSON opcionales (logros, dificultades,
  etiquetas), más un índice para el listado por perfil y rango de fechas.
- **AÑADIDO — Modelo `JournalEntry` y `enum Mood` estricto.** `mood` SHALL serializarse en
  `snake_case` (`great`/`good`/`neutral`/`bad`/`terrible`); un valor distinto NO SHALL ser válido ni
  tragarse el dato. La clave pública de la entrada SHALL ser **la fecha**; el `id` es interno.
- **AÑADIDO — `JournalRepo`** con `upsert` por `(profile_id, date)`, `get` por `(profile_id, date)`,
  `list` por rango de fechas con tope y `delete` por `(profile_id, date)`.
- **AÑADIDO — La herramienta `journal`** con las operaciones `get_entry` (de una fecha, o de hoy),
  `write_entry` (crea o sobrescribe la del día), `update_entry` (parcial), `delete_entry` y
  `list_entries` (por rango). Leer y escribir NO piden confirmación; **borrar SÍ**
  (`ExplicitApproval`).
- **MODIFICADO — La herramienta entra en la skill existente de notas (`recuerdos`)** sin estrenar
  una skill nueva (decisión D10); sus `criteria` SHALL incluir el habla de la bitácora («bitácora»,
  «diario», «cómo fue mi día»). El enrutado se medirá con el arnés del repositorio.
- **AÑADIDO — Endpoints de sesión** `GET /api/journal`, `GET /api/journal/{date}`,
  `PUT /api/journal/{date}` y `DELETE /api/journal/{date}`. Una fecha mal formada o un ánimo fuera
  del conjunto son `400`; lo que no existe es `404`.
- **AÑADIDO — Sección de Bitácora en la app:** entrada en la navegación, vista con listado de más
  reciente a más antigua (fecha, ánimo, relato, etiquetas, logros, dificultades), edición y borrado
  con confirmación, y estados de carga, vacío y error; textos en español y tipos en
  `frontend/src/types/`.

## Capabilities

### New Capabilities

- `tools/journal`: la herramienta `journal` del agente —sus cinco operaciones, su contrato y sus
  permisos—. 3 requisitos nuevos.
- `journal-api`: los endpoints de sesión de la bitácora sobre `/api/journal`. 1 requisito nuevo
  (listado y lectura) más el contrato de escritura y borrado.
- `frontend/journal`: el tipo `JournalEntry`, los métodos de cliente, la vista de Bitácora y su
  entrada en la navegación. 3 requisitos nuevos.

### Modified Capabilities

- `db/schema`: **1 requisito nuevo** — la migración que crea `journal_entries` con su unicidad por
  perfil y día y su índice de listado.
- `db/repos`: **2 requisitos nuevos** — el contrato del modelo `JournalEntry`/`Mood` y las
  operaciones de `JournalRepo`.
- `orchestrator/skill-router`: **1 requisito modificado** — el catálogo cerrado de skills, porque
  `recuerdos` pasa a cubrir también `journal` y sus `criteria` ganan el habla de la bitácora. Se
  copia el requisito entero (con sus nueve escenarios) para que el `archive` no pierda ninguno.
- `tools/registry`: **1 requisito modificado** — el catálogo de herramientas integradas, que pasa a
  incluir `journal` (se copian sus tres escenarios vigentes).

## Impact

- **Base de datos:** migración nueva `migrations/20261018000001_journal_entries.sql` (tabla
  `journal_entries` + índice).
- **Backend:** `src/models/journal.rs` (modelo y `Mood`), `src/db/repos/journal.rs` (`JournalRepo`,
  más `pub mod journal` en `src/db/repos/mod.rs`), `src/tools/journal.rs` (la herramienta, más
  `pub mod journal` en `src/tools/mod.rs`), `src/lib.rs` (`build_tool_registry` registra `journal`;
  `app_with_state` fusiona las rutas), `src/routes/journal.rs` y `src/handlers/journal.rs`, y
  `src/orchestrator/skills.rs` (`recuerdos` cubre `journal` y sus `criteria`).
- **Frontend:** `types/index.ts` (tipo `JournalEntry` y `Mood`), `api/client.ts` (métodos), una vista
  de Bitácora nueva, su test colocado, y `AppLayout.tsx` (entrada en la navegación).
- **Spec:** deltas en `db/schema`, `db/repos`, `orchestrator/skill-router`, `tools/registry` y las
  tres capacidades nuevas (`tools/journal`, `journal-api`, `frontend/journal`).
- **Sin dependencias nuevas.** Sin cambios en `settings`, sin claves nuevas, sin tocar el esquema de
  otras tablas.

## Non-goals

Todo lo siguiente vive en el change siguiente, `journal-worker`, y NO se implementa aquí:

- **El worker autónomo** que escribe la bitácora solo (worker propio, una vez al día).
- **El prompt** de la bitácora y su siembra editable.
- **Los ajustes de encendido, hora y modelo** (nace encendida, hora configurable, modelo por
  entorno).
- **La marca de día juzgado** («esa jornada no tiene nada que contar»).
- **El resumen semanal** (`JournalDigest`) y la herramienta de «insights».
- **Los avisos por push** al escribir una entrada y **el clima** (`weather_context`).

Además, este change NO toca `docker-compose.prod.yml`, NO migra datos existentes (no hay ninguno que
conservar) y NO expone ninguna operación destructiva sin confirmación.
