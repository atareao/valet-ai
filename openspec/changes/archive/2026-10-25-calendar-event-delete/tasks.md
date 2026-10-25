# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + specs + tasks) y **OK explícito** del usuario antes de escribir código o tests.

## 1. RED — tool `calendar.get_events`

- [x] 1.1 Test que falla: `get_events` con `start` + `end` devuelve los eventos del rango completo (`end` se respeta), no solo el primer día.
- [x] 1.2 Test que falla: si vienen `start`, `end` y `duration`, **`end` tiene precedencia**.
- [x] 1.3 Test que falla: si solo viene `start` (sin `end` ni `duration`), se mantiene el rango por defecto de 1440 minutos (compatibilidad).
- [x] 1.4 Test que falla: un `end` con formato inválido produce `ToolError::InvalidArguments`.
- [x] 1.5 Test que falla: el esquema de `parameters()` declara `end`.

## 2. GREEN — tool `calendar.get_events`

- [x] 2.1 `src/tools/calendar.rs`: `get_events` usa `end` como límite superior si viene (con precedencia sobre `duration`), conserva `duration` por defecto 1440 y valida el formato de `end`.
- [x] 2.2 `parameters()`: añadir `end` con descripción (ISO 8601, alternativa a `duration` en `get_events`); aclarar que `title` es de `create_event`.

## 3. RED — frontend (borrado)

- [x] 3.1 Test que falla: al confirmar el borrado en el detalle de un evento se llama a `deleteEvent` con el `id` y se recarga el listado.
- [x] 3.2 Test que falla: si `deleteEvent` rechaza, se muestra un aviso de error y el modal de detalle **sigue abierto** (no se recarga ni se finge el borrado).

## 4. GREEN — frontend (borrado)

- [x] 4.1 `frontend/src/components/CalendarView.tsx`: conectar el borrado a `useDeleteEvent().delete` (llamar a la API, cerrar el detalle y `refetch()` al éxito; aviso de error y modal abierto al fallo).

## 5. Verificación y cierre

- [x] 5.1 `cargo test` en verde.
- [x] 5.2 `cargo clippy --all-targets -- -D warnings` (0 warnings).
- [x] 5.3 `cargo fmt --check` en verde.
- [x] 5.4 `npx tsc --noEmit` y `npx vitest run` en verde.
- [x] 5.5 `openspec validate calendar-event-delete --strict` en verde.
- [x] 5.6 Revisión con `@rust-reviewer` (backend) y `@react-reviewer` (UI); hallazgos aplicados (ver §6).

- [ ] 5.7 `openspec archive calendar-event-delete` y registro del cierre en `AGENTS.md` §V si procede.

## 6. Hallazgos de auditoría (aplicados)

- [x] 6.1 **Backend (menor).** `get_events` SHALL normalizar **ambos** límites del rango a UTC antes
  de compararlos: `list_by_date_range` compara strings lexicográficamente y los eventos se guardan
  en UTC con `Z`, así que un `start`/`end` con offset (`+02:00`, `-05:00`…) daría un rango erróneo.
  Dos tests nuevos (RED→GREEN): `test_get_events_normalizes_end_with_negative_offset`,
  `test_get_events_normalizes_start_with_positive_offset`.
- [x] 6.2 **Frontend (nit).** El caso feliz del test SHALL assertar que el detalle se cierra.
  En jsdom el `Modal` de antd no desmonta el nodo (no se emite `transitionend`; precedente en
  `SettingsDialog.test.tsx`), así que se usa la señal observable equivalente de `rc-motion`
  (el nodo pasa a `ant-zoom-leave` al procesar `open=false`); la aserción falla si el detalle
  siguiera abierto.
