# Change: Integrar OpenTodo como backend de tareas del agente

## Why

Valet gestiona hoy las tareas en una tabla SQLite propia (`tasks` + `tasks_fts`) con una herramienta
`tasks` de cinco operaciones y una vista `TaskView` en el frontend. El usuario trabaja en **OpenTodo**
(`https://todo.territoriolinux.es`), un GTD con proyectos, etiquetas, espacios de trabajo y vistas
calculadas. Mantener dos listas obliga a duplicar y la local no tiene ni proyectos ni vistas: el
agente debe pasar a operar sobre OpenTodo.

Los hechos que condicionan el diseño se verificaron contra el **servidor real** (v0.16.0, no la
documentación pública, que anuncia v0.13.0) el 2026-10-27, con un token `scope: full`:

- `GET /api/v1/tasks` **no filtra**: solo acepta `include_deleted` y `completed_after/before`. Las
  vistas (Bandeja de entrada, Hoy, Próximo, En cualquier momento, Algún día, En espera) son
  **predicados del cliente** y hay que recalcularlas en Rust. Hoy devuelve 3 tareas; mañana devolverá
  miles.
- `?workspace_id=` **se ignora** (probado con un UUID inventado y con un parámetro cualquiera).
- `priority` es `1..4` con **4 por defecto** (1 es la más alta): no existe «sin prioridad».
- Las fechas son `{date, time?, tz?}`; las marcas de tiempo van en **milisegundos Unix**; los campos
  ausentes no vienen en el JSON (p. ej. `start`, `deadline`, `evening`).
- `PATCH /api/v1/tasks/{id}` con `labels` **reemplaza el array entero**: enviar solo la etiqueta de
  estado borra las demás.
- Un `Idempotency-Key` repetido responde **409** y no devuelve la tarea original.
- `POST /api/v1/quick-add/parse` resuelve fechas naturales en español, pero **no reconoce
  prioridad**.
- `DELETE /api/v1/labels/{id}` responde **204** y solo pone lápida; `POST /api/v1/labels` acepta
  `workspace_id` y la crea en ese espacio. Los nombres de etiqueta **no son únicos** (unicidad
  «advisory»).
- `DELETE /api/v1/tasks/{id}/purge` existe, responde **200** con `{"purged":{…}}` y es definitivo; sin residuo.

## What Changes

Este change es la **fase 1**, que solo **añade**. La retirada del TODO local es un change posterior
(`remove-local-tasks`), de modo que la integración se valida en producción sin perder nada.

- **Nueva capa de servicio** `src/services/opentodo.rs` (molde `zennotes.rs` / `miniflux.rs`): URL
  base y token resueltos **en cada llamada** (ajustes → entorno → valor por defecto para la URL),
  `reqwest` con timeout de 10 s, errores tipados (`NotConfigured`, `Unauthorized`, `NotFound`,
  `Upstream`, `InvalidInput`) y **nunca** el token en logs ni en mensajes. Incluye la deserialización
  tolerante (todo `Option`/`#[serde(default)]`), las **vistas calculadas en Rust**, la resolución de
  proyectos y etiquetas por nombre y la operación `set_label`.
- **Herramienta `tasks` con contrato nuevo** (mismo nombre, misma skill): operaciones `list_tasks`,
  `search_tasks`, `add_task`, `update_task`, `complete_task`, `delete_task` y `set_label`, sobre la
  API de OpenTodo. `list_tasks` **nunca** devuelve la lista cruda: como máximo `limit` tareas (def.
  20, tope 50) con campos mínimos, más un **resumen agregado** por vista y por proyecto.
- **Compatibilidad hacia atrás**: si `opentodo_token` no está configurado, `tasks` conserva el
  comportamiento local actual (tabla `tasks`, repositorio y `profile_id`), de modo que desplegar
  este change **no cambia nada** hasta que el usuario pegue el token. Con token, todas las
  operaciones van a OpenTodo.
- **Ajustes y endpoints dedicados**: `opentodo_base_url` (por defecto
  `https://todo.territoriolinux.es`) y `opentodo_token`. El token entra en `SENSITIVE_KEYS`, así que
  `GET /api/settings` lo omite y `PUT /api/settings` lo ignora; la única puerta es
  `PUT /api/opentodo/token` (204/400) y el estado se consulta en `GET /api/opentodo/status`
  (`{configured, base_url}`), que **nunca** devuelve el token. Mismo contrato que ZenNotes.
- **Cuatro etiquetas, dos grupos excluyentes**, decididas por el usuario y **sin prefijo `@`**:
  estado (`doing`, `waiting`) y ámbito (`shared`, `personal`). Solo se etiqueta lo que no tiene
  campo equivalente; `inbox`, `someday`, `done` y las fechas se escriben con los campos que la app
  ya gestiona. La escritura va **siempre** por `set_label` (grupo + valor), que lee la tarea, quita
  las etiquetas de su grupo, añade la nueva y conserva el resto: el modelo nunca maneja ids a pelo.
- **Un solo espacio de trabajo**: el de por defecto. No hay ajuste de espacio ni parámetro
  `workspace`; los `POST` omiten `workspace_id`. Al leer no se filtra por espacio (ver riesgos).
- **Fragmento de prompt**: migración nueva e idempotente que actualiza `SKILL_PENDIENTES_PROMPT` para
  que explique las vistas, la prioridad 1–4, `start: "someday"`, proyectos y etiquetas, y que las
  tareas (OpenTodo) siguen siendo distintas de los recordatorios de Valet. Nunca pisa una edición del
  usuario.
- **Frontend**: octava sub-pestaña en «Integraciones», con el patrón ya estrenado
  (`KEY_INTEGRATIONS` + sección propia), campo de token que **arranca siempre vacío**, indicador
  «Configurado / Sin configurar» y botón «Probar conexión» contra `GET /api/opentodo/status`.

## Impact

### Specs
- `tools/tasks` — **REMOVED** de los dos requisitos vigentes (contrato local de cinco operaciones y
  permisos) y **ADDED** de los requisitos nuevos (contrato OpenTodo, vistas calculadas, recorte y
  resumen, `set_label`, permisos).
- `opentodo-service` — **nueva**: configuración, cliente, vistas, etiquetas y `set_label`.
- `opentodo-api` — **nueva**: ajustes, sensibilidad del token, endpoints dedicados y migración
  (ajustes + fragmento de prompt).
- `frontend` — **MODIFIED** de los dos requisitos de «Integraciones» (octava sub-pestaña y criterio
  del indicador).

### Código
- `src/services/opentodo.rs` (nuevo), `src/services/mod.rs`.
- `src/tools/tasks.rs` (reescritura del despacho con doble backend), `src/tools/mod.rs`.
- `src/db/repos/settings.rs` (`SENSITIVE_KEYS`).
- `src/handlers/opentodo.rs` y `src/routes/opentodo.rs` (nuevos), `src/routes/mod.rs`.
- `frontend/src/components/keyIntegrations.ts` (definición nueva), `frontend/src/components/OpentodoSection.tsx`
  (nuevo), `frontend/src/hooks/useOpentodo.ts` (nuevo), `frontend/src/components/IntegrationsTab.tsx`,
  `frontend/src/types.ts`.
- `migrations/20261027000001_opentodo.sql` y su target de test.

### Datos
- Solo una migración aditiva: siembra dos ajustes y promueve el fragmento si sigue siendo el sembrado.
- **No se borra la tabla `tasks`**, ni `tasks_fts`, ni la API local, ni la vista del frontend.

## Non-goals

- **No se retira el TODO local**: es el change siguiente (`remove-local-tasks`), con su spec de
  caracterización y su volcado de datos.
- **Vista `Next`**: depende del método de trabajo y de una agrupación por contexto no documentada
  como algoritmo. Fuera del alcance.
- **Vistas guardadas**: Valet no las crea; la consulta (`label:doing`) se deja documentada para que
  el usuario las fije él.
- **Múltiples espacios de trabajo**: solo el de por defecto, sin filtro al leer.
- **MCP**: esto es integración nativa por HTTP (§V.9 de `AGENTS.md`).
- **Cifrado de OpenTodo**: la instancia lo tiene desactivado; Valet no lo gestiona.
- **Sub-tareas, recurrencia, estimaciones y `evening`**: existen en la API pero no se exponen en el
  contrato de la herramienta en este primer alcance.

## Riesgos

- **R1 (alto) Volumen**: `GET /tasks` no filtra y crecerá sin límite. Mitigación: agregados y
  `limit` duro en la herramienta, con test que garantiza el tope.
- **R2 (alto) Etiquetas**: el `PATCH` reemplaza el array. Mitigación: `set_label` en el servicio,
  test que verifica que no borra etiquetas ajenas.
- **R3 (medio) Dependencia externa**: si OpenTodo cae, el turno no debe caer. Mitigación: timeout de
  10 s y error legible; el backend local sigue disponible mientras no haya token.
- **R4 (medio) Token `full`**: puede borrar y purgar. Mitigación: `ExplicitApproval` también en
  `delete_task` con `purge`, token en `SENSITIVE_KEYS` y endpoint dedicado.
- **R5 (bajo) Deriva de esquema** entre v0.16.0 y versiones futuras: deserialización tolerante
  (`Option` + `default`) y test con la forma real de una tarea.
- **R6 (bajo) Otros espacios de trabajo**: una tarea creada a mano en «Trabajo» aparece en los
  listados. Aceptado; filtrar sería una línea por `workspace_id` si algún día se decide.
