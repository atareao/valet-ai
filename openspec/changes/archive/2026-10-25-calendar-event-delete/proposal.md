# Change: borrado de eventos del calendario (UI + `get_events`)

## Why

Borrar eventos del calendario no funciona por **ninguna** de las dos vías, y cada una tiene una
causa distinta y real:

1. **A mano (UI).** `CalendarView` conecta el borrado a `handleEventDeleted`, que solo cierra el
   modal de detalle y recarga el listado: **nunca llama a la API de borrado**. El hook
   `useDeleteEvent` y `api.deleteEvent` existen, pero no se usan en ninguna vista. Pulsar «Delete»
   no envía ningún `DELETE`.

2. **Por Valet (chat).** La tool `calendar` expone `get_events` con `start` + `duration` e
   **ignora `end`** (no lo declara en el esquema). El LLM pide rangos con `start` + `end` —lo
   natural, y así aparece en los logs de producción— y el rango se colapsa al `duration` por
   defecto (1440 min = 1 día). El agente ve listados incompletos, se contradice («0 eventos» /
   «25 eventos» / «no he borrado nada») y **nunca resuelve el `id`** que `delete_event` exige, así
   que no llega a borrar. En los logs de `co1` de esos intentos **no hay ni una sola llamada a
   `delete_event`**.

Evidencia recogida de los logs y la base de datos de producción (`co1`, contenedor `valet`).

## What Changes

- **Frontend (`frontend/agenda`)**: al confirmar el borrado en el detalle de un evento, la vista
  SHALL llamar a `api.deleteEvent(id)` (a través de `useDeleteEvent`), SHALL recargar el listado al
  terminar y SHALL mostrar un aviso de error **sin** cerrar el modal si el borrado falla.
- **Tool (`tools/agenda`)**: `get_events` SHALL aceptar `end` como alternativa a `duration` para
  acotar el rango (`end` tiene precedencia si vienen ambos; si no viene ninguno, `duration` sigue
  valiendo 1440). El parámetro `end` SHALL declararse en el esquema de `parameters()`. El
  comportamiento de `create_event`/`update_event` con `start` + `duration` no cambia.

## Impact

- **Specs afectadas**: `frontend/agenda` (modificada), `tools/agenda` (modificada).
- **Código**:
  - `frontend/src/components/CalendarView.tsx` (conectar el borrado + manejo de error).
  - `src/tools/calendar.rs` (`get_events` acepta `end`; esquema en `parameters()`).
- **Sin migración de base de datos.** Sin cambios en la API HTTP (`events-api`): `DELETE
  /api/events/{id}` ya existe y funciona.
- **No-goals (fuera del alcance)**:
  - Buscar eventos por título en `get_events` (queda como posible mejora futura).
  - Saneado de los duplicados exactos (mismo título + mismo día) ya presentes en los datos.
  - Cualquier cambio en `docker-compose.prod.yml`.
