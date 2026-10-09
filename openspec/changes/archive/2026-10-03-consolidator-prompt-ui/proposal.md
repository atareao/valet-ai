# Proposal: El prompt del consolidador en la pestaña de Prompts

## Why

La Capa C se consolida con `settings.consolidator_prompt`, que la migración
`20261002000001_persistent_memory.sql` siembra pero que **no se puede ver ni editar desde ningún
sitio**: para cambiarlo hay que tocar la base de datos a mano. El resto de prompts que gobiernan
el comportamiento del sistema —`system_prompt`, `archivist_prompt` y `collapse_prompt`— sí están
en la pestaña «Prompts»; el del consolidador es el único que falta, y es precisamente el que
decide qué hechos se conservan en la memoria persistente. Una memoria que no se puede inspeccionar
ni corregir no es confiable, y su prompt tampoco.

## What Changes

- **D1 — Sub-pestaña «Consolidator».** La pestaña «Prompts» del diálogo de ajustes gana una
  sub-pestaña que muestra el valor vigente de `settings.consolidator_prompt` en un área de texto
  editable, junto a las ya existentes *System*, *Archivist* y *Collapse*.
- **D2 — Guardado por el endpoint de settings.** El valor se persiste con el endpoint de settings
  ya existente, sin tocar el estado persistente ni el presupuesto de tokens. No hay cambios de
  backend: `PUT/GET /api/settings` ya tratan cualquier clave.
- **D3 — Aviso de placeholders.** El prompt del consolidador se usa como mensaje de sistema con
  `{{ ESTADO_ACTUAL }}` y `{{ BLOQUE_DE_MENSAJES }}` sustituidos en caliente. El fallback del
  worker solo actúa si el prompt está vacío o ausente, no si está mal formado. Al guardar, la
  interfaz SHALL avisar —sin bloquear— si falta alguno de los dos placeholders, nombrando los
  ausentes, y SHALL permitir guardar igualmente.

### Fuera de alcance

- Exponer en la UI los knobs del worker de memoria (`MEMORY_BATCH_TOKENS`,
  `MEMORY_INACTIVITY_MINUTES`, `MEMORY_OVERLAP`, `MEMORY_POLL_INTERVAL_MINUTES`).
- Elegir `COLLAPSE_MODEL` / `MEMORY_MODEL` / `SEMANTIC_MODEL` desde la interfaz.
- Validación en backend de los placeholders.
- Tocar `docker-compose.prod.yml`.

## Capabilities

### New Capabilities

- `consolidator-prompt-ui`: la edición desde la pestaña Prompts del prompt del consolidador de la
  Capa C, con aviso no bloqueante de placeholders ausentes.

### Modified Capabilities

- (ninguna)

## Impact

- **Specs**: `consolidator-prompt-ui` (nueva).
- **Frontend**: `types/index.ts` (clave `consolidator_prompt` en el tipo de settings),
  `components/SettingsDialog.tsx` (tipo del formulario, carga, payload de guardado y sub-pestaña),
  y sus tests.
- **Backend**: sin cambios (`PUT/GET /api/settings` ya son genéricos).
- **Sin migración** (la clave ya está sembrada). **Sin tocar** `docker-compose.prod.yml`.
