# Design — borrado de eventos del calendario

## Contexto

Dos rutas de borrado, dos causas:

| Vía | Quién | Causa raíz | Fichero |
|---|---|---|---|
| A mano | UI (modal de detalle del evento) | El `onDelete` no llama a la API: `handleEventDeleted` solo cierra y recarga | `frontend/src/components/CalendarView.tsx` |
| Por Valet | Tool `calendar` | `get_events` ignora `end`; el rango se colapsa a `duration`=1440 → el agente no ve los eventos y no obtiene el `id` | `src/tools/calendar.rs` |

El backend de borrado (`DELETE /api/events/{id}`, `EventsRepo::delete`) funciona y está cubierto por
tests. No hay triggers rotos en `events_fts` (es de contenido externo y está sincronizada; no rompe
el `DELETE`).

## Decisiones

### D1 — Frontend: conectar el borrado con el hook existente

`useDeleteEvent` ya existe (`useEvents.ts`) y `api.deleteEvent` también. La vista SHALL usarlo:

- `EventDetail.onDelete(id)` sigue siendo el contrato del componente (ya está bien tipado y
  probado como prop).
- `CalendarView` SHALL crear el handler con `useDeleteEvent().delete`, llamar a la API con el `id`
  y, al terminar con éxito, cerrar el detalle y `refetch()`.
- Si el borrado **falla**, SHALL mostrarse un aviso (`message.error` de antd, patrón ya usado en el
  repo) y el modal de detalle SHALL permanecer abierto. No se recarga para no dar la falsa
  sensación de que se borró.

Alternativa descartada: llamar a `api.deleteEvent` directamente desde `EventDetail`. Se mantiene la
separación vista/efecto: el detalle es presentacional, la vista orquesta los efectos.

### D2 — Tool: `get_events` acepta `end` además de `duration`

El LLM expresa rangos con `start` + `end`. En vez de “educar” al modelo (frágil), la tool SHALL
aceptar ambas formas:

- Si viene `end` (ISO 8601 válido), SHALL usarse como límite superior del rango (`start ..= end`).
- Si vienen `start` + `duration`, se mantiene el comportamiento actual (`end = start + duration`).
- Si vienen ambos, `end` tiene precedencia (es lo más explícito).
- Si no viene ni `end` ni `duration`, `duration` SHALL seguir valiendo 1440 (compatibilidad).

Se descarta **sustituir** `duration` por `end`: hay llamadas y tests que usan `duration` y el
esquema declara ese parámetro; quitarlo rompería compatibilidad sin ganancia.

### D3 — No se reintroduce `date` ni se cambia `create_event`/`update_event`

El requisito «unified schema» del que cuelga esto prohibía `end`; se modifica **solo** para el rango
de `get_events`. `create_event`/`update_event` siguen con `start` + `duration` para calcular el fin
del evento (no es un rango de consulta).

### D4 — `title` NO es filtro de `get_events`

El esquema ya describe `title` como «obligatorio en `create_event`». En los logs el modelo lo usa
para “buscar por título” y obtiene resultados sin filtrar. Se aclara en la descripción que `title`
es de `create_event`; **no** se añade búsqueda por título en este change (fuera de alcance).

## Riesgos

- **Bajo.** Sin migración ni cambios de contrato HTTP. El cambio del frontend es de cableado; el de
  la tool, aditivo (`end` opcional). Los tests de `get_events` con `duration` deben seguir verdes.
- El aviso de error del frontend evita el fallo silencioso; el modal que no se cierra es el
  comportamiento deseado (el usuario ve que no se borró).

## Verificación

- Backend: `cargo test` (incluye un test nuevo de `get_events` con `end`), `clippy -D warnings`,
  `fmt --check`.
- Frontend: `vitest run` (test nuevo del borrado con confirmación → `deleteEvent` llamado; y el de
  fallo → aviso y modal abierto), `tsc --noEmit`, `eslint`.
