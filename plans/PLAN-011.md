# PLAN-011 — Integrar OpenTodo y retirar el TODO local

> Estado: **fase 1 cerrada**. Change `opentodo-integration` aprobado, validado, implementado, auditado y archivado; fusionado en `development` con el PR #265 (`99383c7`). Queda pendiente la verificación E2E real, **aplazada al despliegue**.
> Convención de repo: si se aprueba se renombra a `plans/PLAN-011.md`; al completarse, a `plans/PLAN-011-COMPLETED.md`.
> Origen: petición del 2026-10-27 («quitar el TODO actual e integrar OpenTodo»), con cuatro decisiones tomadas por el usuario y una batería de pruebas contra la instancia real.

## 0. Objetivo

Que el agente de Valet gestione las tareas del usuario **en OpenTodo** (`https://todo.territoriolinux.es`) en vez de en la tabla local `tasks`, y retirar el TODO propio de Valet: tabla, API, herramienta del agente y vista del frontend.

## 1. Decisiones tomadas por el usuario

1. **La vista de tareas de Valet desaparece.** `TaskView` se sustituye por un acceso directo a `https://todo.territoriolinux.es`. No se reescribe la interfaz contra la API de OpenTodo.
2. **Se permite escritura** desde Valet (crear, editar, completar y borrar tareas).
3. **Una sola cuenta.** OpenTodo ya tiene proyectos, etiquetas y **espacios de trabajo**; la integración debe poder apuntar a un espacio de trabajo.
4. **Los «estados» que el usuario espera** —Bandeja de entrada, Hoy, Próximo, En espera, En cualquier momento, Algún día— existen en OpenTodo, pero **no como campo**: son vistas calculadas en el cliente a partir de los campos. La integración debe reproducir sus reglas (ver §3).

## 2. Hechos verificados contra la instancia real (2026-10-27)

Instancia: `GET /healthz` → `{"status":"ok","version":"v0.16.0"}`. La documentación pública se anuncia como v0.13.0, **así que las pruebas se hicieron contra el servidor, no contra la doc**.

| Comprobación | Resultado |
|---|---|
| `GET /api/v1/me` | token `scope: "full"`, cuenta `owner`, `encryption.enabled: false`, `agent_excluded_projects: []` |
| `GET /api/v1/tasks` | **3 tareas**, respuesta de 1.029 bytes |
| `GET /api/v1/projects` | **0 proyectos** |
| `GET /api/v1/labels` | **0 etiquetas** |
| `GET /api/v1/views` | **0 vistas guardadas** (las integradas no las devuelve el servidor) |
| `GET /api/v1/workspaces` | **2 espacios**: `Personal` (`default: true`, `id f9235de2-…`) y **`Trabajo`** (`id 5ed1f72c-…`), ambos con `role: owner` |
| `POST /api/v1/tasks` | **201**, acepta `title`, `description`, `priority` numérica, `due {date,time,tz}`, `start:"someday"`, `labels` |
| `PATCH /api/v1/tasks/{id}` | **200**, acepta `evening`, `estimate_minutes` |
| `DELETE /api/v1/tasks/{id}` | **204** (borrado suave, a la papelera) |
| `DELETE /api/v1/tasks/{id}/purge` | **200** `{"purged":{"tasks":1,…}}`; sin residuo, la cuenta vuelve a sus 3 tareas |
| Reintento con el mismo `Idempotency-Key` | **409** (no devuelve la tarea original: hay que tratar el 409 como «ya aplicado») |
| `POST /api/v1/quick-add/parse` «llamar al fontanero el viernes a las 10 #casa» | `due {date:"2026-10-30", time:"10:00"}` correcto; `#casa` → `unmatched`, se queda en el título |
| `POST /api/v1/quick-add/parse` «revisar la factura de la luz mañana» | `due {date:"2026-10-28"}` correcto |
| `POST /api/v1/quick-add/parse` «… para el lunes prioridad alta» | fecha correcta, **`prioridad alta` no se reconoce**: se queda en el título y no produce `priority` |
| `GET /api/v1/settings/mcp` | `enabled: false`, `writes_enabled: false` — irrelevante con token `full` |
| `POST /mcp` | 404 (MCP desactivado; no se usa) |
| `GET /api/v1/tasks?workspace_id=<id de Trabajo>` | **3 tareas: el parámetro se ignora**. Con un UUID inventado y con `?basura=xyz` devuelve también 3 |
| Forma de las tareas | cada tarea lleva `workspace_id` (`null` = espacio por defecto); una tarea con proyecto sigue el espacio del proyecto |
| `POST /api/v1/labels` | **201** `{id,name,color,created_at,position}`; el servidor **no** impone unicidad de nombre (es «advisory») |
| `PATCH /api/v1/tasks/{id}` con `labels` | **reemplaza el array entero**: enviar sólo `["<@doing>"]` borra las demás etiquetas de la tarea |
| `PATCH` con un id de etiqueta inventado | **400** `invalid_input`: `task.labels references unknown or deleted label` (no falla en silencio) |
| `DELETE /api/v1/labels/{id}` | **204**; sólo pone lápida: las tareas que ya llevaban el id lo conservan y la app los oculta |
| `POST /api/v1/labels` con `workspace_id` | **201** y la etiqueta nace en ese espacio (`{id, name, workspace_id}`); `GET /api/v1/labels` devuelve el `workspace_id` de cada una |

Forma real de una tarea devuelta (campos ausentes cuando no están puestos, p. ej. `start`, `deadline`, `evening`, `recurrence`):

```json
{"id":"…","title":"Ejemplo","description":"","project_id":null,"parent_id":null,
 "priority":4,"due":{"date":"2026-10-27"},"labels":[],"fields":{},"position":1793082844171,
 "created_at":1793082844171,"completed_at":null,"completed_by":null,"deleted_at":null,
 "estimate_minutes":null,"origin":null}
```

**Consecuencias directas:** toda la deserialización usa `Option<T>` y `#[serde(default)]`; `priority` es 1–4 (1 la más alta) y **no** es `low/medium/high`; las fechas son `{date, time?, tz?}` y las marcas de tiempo van en **milisegundos Unix**.

## 3. Las vistas: no son un campo, son predicados

Reglas tomadas de `https://open-todo.app/docs/built-in-views/` y `https://open-todo.app/docs/method/places/`, que la herramienta debe **recalcular en Rust**:

| Vista | Predicado |
|---|---|
| Bandeja de entrada (Inbox) | abierta (`completed_at == null`) y `project_id == null` |
| Hoy (Today) | abierta y (`due.date <= hoy` o `deadline <= hoy`); las anteriores primero |
| Próximo (Upcoming) | `due.date > hoy`, más las sin fecha con `deadline` por delante, agrupadas por día |
| En cualquier momento (Anytime) | abierta y sin `due`, sin `deadline` y sin `start` |
| Algún día (Someday) | `start == "someday"` |
| En espera (Waiting) | abierta y (`assignee_id != yo` o `blocked_by` no vacío) |
| Siguiente (Next) | la «próxima acción» de cada proyecto, agrupada por contexto (`labels` que empiezan por `@`) |

`Next` depende del método de trabajo y de una agrupación por contexto que no está documentada como algoritmo; **queda fuera del primer alcance** y se documenta como tal. Las demás se implementan y se prueban con vectores fijos, porque el servidor **no filtra**: `GET /api/v1/tasks` solo acepta `include_deleted` y `completed_after`/`completed_before`, y el resto de filtros «se evalúan en cada dispositivo».

Lo mismo ocurre con el espacio de trabajo: `?workspace_id=` en `/tasks` y `/projects` no filtra nada (probado con un UUID inventado y con un parámetro cualquiera: devuelve lo mismo), así que **el filtrado por espacio también se hace en Valet**, comparando `workspace_id` y resolviendo que una tarea con proyecto hereda el espacio del proyecto.

Los estados y ámbitos que no tienen campo se modelan con **etiquetas** (ver §5), no con predicados nuevos: `doing` pasa a ser «lleva la etiqueta `doing`», `waiting` «lleva `waiting`, o `blocked_by` no vacío, o `assignee_id` distinto del mío», y el antiguo `scope` «lleva `shared` o `personal`». Los nombres los decidió el usuario y van **sin prefijo `@`**.

## 4. Mapeo desde el TODO local

| Valet hoy (`tasks`) | OpenTodo | Nota |
|---|---|---|
| `content` | `title` + `description` | — |
| `status: inbox` | `project_id: null` | coincide |
| `status: todo` | tarea abierta, sin más | no hay estado |
| `status: doing` | **etiqueta `doing`** | una etiqueta expresa el estado sin inventar fechas: la vista Hoy sigue siendo lo que tiene fecha. Sirve de filtro (`label:doing`) y de vista guardada, así que la app puede mostrar lo mismo que el agente |
| `status: waiting` | **etiqueta `waiting`** (más `blocked_by` cuando el bloqueo sea otra tarea) | cubre lo que espera a una **persona**, que la vista En espera no alcanza; al leer, las dos señales se suman |
| `status: someday` | `start: "someday"` | — |
| `status: done` | `completed_at` | — |
| `priority: critical/high/medium/low` | `priority: 1..4` | **1:1** decidido por el usuario: `critical → 1`, `high → 2`, `medium → 3`, `low → 4`. Ojo: en OpenTodo **4 es el valor por defecto**, así que no existe «sin prioridad»; toda tarea que no la fije (también las creadas en la app) se lee como `low` |
| `project` (texto libre) | `project_id` (entidad) | se resuelve por **nombre**; si no existe, se devuelve la lista real de proyectos y **no** se inventa un id |
| `due_date` | `due: {date, time?, tz?}` | acepta hora y zona |
| `scope: shared/personal` | **etiqueta `shared` o `personal`** | decidido por el usuario. Aviso: la etiqueta **expresa** el ámbito, no lo cambia; compartir de verdad sigue siendo invitar a alguien al proyecto, y una tarea «shared» en un proyecto sin miembros no la ve nadie más |
| `profile_id` | cuenta única | se descarta el concepto de perfil para tareas |

## 5. Diseño propuesto

### Fase 1 — `opentodo-integration` (añadir, sin borrar nada)

- **Cliente HTTP** `src/services/opentodo.rs`, con el patrón de `src/services/zennotes.rs` y `strava.rs`: base URL + token desde `settings`, `reqwest` con timeout de 10 s, errores tipados (`NotConfigured`, `Unauthorized`, `Upstream`, `InvalidInput`) y **nunca** el token en logs ni en mensajes de error.
- **Ajustes nuevos** en `settings`: `opentodo_base_url` (def. `https://todo.territoriolinux.es`) y `opentodo_token`. El token entra en `SENSITIVE_KEYS` (`src/db/repos/settings.rs:13`), así que `GET /api/settings` lo omite y `PUT /api/settings` lo ignora: la única puerta es un endpoint dedicado tipo `PUT /api/opentodo/token` (204/400) con estado en `GET /api/opentodo/status` que **nunca** devuelve el token. Es el mismo contrato que `zennotes`.
- **Sólo el espacio de trabajo por defecto**, decidido por el usuario. No hay ajuste de espacio ni parámetro `workspace` en la herramienta: los `POST` omiten `workspace_id` (que equivale al espacio `default: true`, hoy «Personal») y las etiquetas `doing`, `waiting`, `shared` y `personal` se crean **en ese espacio**. Consecuencia asumida: una tarea creada a mano en otro espacio (p. ej. «Trabajo») sigue apareciendo en los listados, porque leer no se filtra; Valet escribe siempre en el de por defecto. Si algún día se quiere que Valet sea **ciego** a otros espacios, es un filtro de una línea por `workspace_id` y del espacio del proyecto, y por eso queda anotado aquí.
- **Herramienta `tasks` nueva** (misma herramienta, contrato nuevo):
  - `operation`: `list_tasks`, `add_task`, `update_task`, `complete_task`, `delete_task`, `search_tasks`.
  - `list_tasks` con `view` (`inbox|today|upcoming|anytime|someday|doing|waiting|all`) **calculada en Rust** según §3, más `project` y `limit` (def. 20, tope 50).
  - **Guarda de tamaño**: la herramienta nunca devuelve la lista cruda. Devuelve como máximo `limit` tareas (título, id, proyecto, `due`, `priority`, `start`) más un resumen agregado (totales por vista y por proyecto). Es la lección de ZenNotes aplicada aquí: `GET /tasks` no filtra y con años de uso serían megabytes.
  - Al crear o editar, la herramienta **debe** resolver nombres a ids en el mismo turno: `list_projects` y `list_labels` (o el `project_id` que ya trajo `list_tasks`). Las peticiones de proyecto/etiqueta inexistentes se responden con la lista real, no se inventan ids.
  - `delete_task` mantiene `Permission::ExplicitApproval`; se añade **también** a `delete` definitivo (`purge`), que hoy no existe en el TODO local. El token `full` puede purgar, así que la salvaguarda es de la herramienta, no del servidor.
  - `due` se maneja como `{date, time?, tz?}`; la conversión de expresiones naturales («el viernes a las 10») **se delega en `POST /api/v1/quick-add/parse`** y no en el modelo, salvo para la prioridad, que quick-add no reconoce y hay que pasar explícita. La prioridad se escribe siempre explícita con esos cuatro valores, porque el 4 (que es el defecto del servidor) equivale a `low`.
- **Etiquetas de estado y de ámbito, y sólo esas.** `inbox`, `someday`, `done` y «sin fecha» ya se expresan con campos que la app escribe sola (park con `n`, completar, `due`), así que etiquetarlos crearía dos fuentes de verdad que se desincronizan en cuanto el usuario toque la app. Se usan **cuatro etiquetas en dos grupos mutuamente excluyentes**: estado (`doing`, `waiting`) y ámbito (`shared`, `personal`). Van sin `@` por decisión del usuario: al no empezar por `@` no se tratan como contextos en la vista Next, que es lo deseable para un estado.
- **Regla de escritura de etiquetas** (el footgun verificado): `PATCH /tasks/{id}` con `labels` **reemplaza el array entero**, así que la fusión la hace el **servicio** y nunca el modelo. Una única operación `set_label` con `group` (`state`|`scope`) y `value` (`doing`|`waiting`|`shared`|`personal`|null) que lee la tarea, quita las etiquetas **del mismo grupo**, añade la nueva, conserva todas las demás y envía la lista completa. El modelo no maneja ids de etiquetas a pelo.
- **Resolución y creación de etiquetas**: `GET /api/v1/labels` por espacio de trabajo, caché por turno y coincidencia por nombre exacto (el servidor acepta duplicados). Si falta alguna de las cuatro, se crea en ese espacio la primera vez; si el usuario la borró a propósito, no se recrea en cada turno: la herramienta lo dice. Y como `shared` y `personal` son palabras comunes, si ya existe una etiqueta con ese nombre creada por el usuario se **adopta** y se avisa, en vez de crear un duplicado.
- **Skill `pendientes`**: se reescribe el fragmento `SKILL_PENDIENTES_PROMPT` (sembrado en `migrations/20261012000003_pendientes_notifications_prompt.sql`, con `prompt_heading` en `src/orchestrator/skills.rs:84`) porque enseña literalmente los estados GTD y que `due_date` «no dispara nada». El nuevo texto debe explicar vistas, prioridad 1–4, `start: someday`, proyectos/etiquetas y que `reminders` (Valet) **sigue siendo otra cosa** que las tareas (OpenTodo). Migración nueva e idempotente, sin sobrescribir un fragmento editado por el usuario.
- **Frontend**: octava sub-pestaña en «Integraciones» con el patrón recién estrenado (`KeyIntegration.tsx` + `KEY_INTEGRATIONS`), campo de token que arranca siempre vacío, indicador Configurada/Sin configurar, y botón «Probar conexión» que llama a `GET /api/opentodo/status`. El TODO local **sigue vivo** aquí: es el fallback mientras se valida la integración.
- **Errores de red**: si OpenTodo no responde, la herramienta devuelve un error legible («OpenTodo no responde: …») y el agente lo dice; un fallo del servicio externo no debe tumbar el turno.

### Fase 2 — `remove-local-tasks` (quitar, solo tras validar la fase 1 en producción)

1. **Phase 0 del AGENTS.md primero**: no existe spec de `tasks` en `openspec/specs/`, así que hay que escribir la spec de caracterización del comportamiento actual **antes** de borrar nada.
2. Volcado de las tareas existentes: exportarlas y crearlas en OpenTodo con `POST /api/v1/tasks` (o, si son muchas y variadas, generar el fichero de importación de OpenTodo y usar su importador). Verificar el recuento antes de borrar.
3. Borrar: tabla `tasks` + `tasks_fts`, `src/db/repos/tasks.rs`, `src/handlers/tasks.rs`, `src/routes/tasks.rs` (y su registro en `src/routes/mod.rs`), `src/tools/tasks.rs` viejo, la dimensión `tasks` de `src/tools/unified_search.rs:63`, las semillas de `src/bin/seed.rs` y los casos de `src/bin/route-eval.rs`.
4. Frontend: eliminar `TaskView.tsx`, `TaskModal`, `useTasks.ts`, el `lazy()` de `AppLayout.tsx:24-25`, el estado `tasksVisible` (`:91`) y su modal (`:221`), el evento `tasks-changed` de `useMainChat.ts:291` y los tres ficheros de test. En su lugar, un acceso directo a `https://todo.territoriolinux.es` (misma plaza en la navegación, icono y `rel="noopener noreferrer"`).
5. Tests de integridad del catálogo de skills y del registro de herramientas: el catálogo exige que toda herramienta registrada pertenezca a una skill, así que la retirada debe dejar el catálogo consistente.

## 6. Riesgos

| Riesgo | Mitigación |
|---|---|
| **`GET /tasks` no filtra**: crecerá sin límite | agregados + `limit` duro en la herramienta; medir en producción |
| El servidor no tiene proyectos ni etiquetas (0 y 0 hoy) | el agente debe crear el proyecto o preguntar; `#nombre` llega como `unmatched` |
| `priority` y fechas difieren del modelo local | mapeo explícito en la herramienta y en el fragmento de prompt; tests con vectores reales |
| Token `full` puede borrar y purgar | `ExplicitApproval` en `delete_task` y en purga; token en `SENSITIVE_KEYS`; endpoint dedicado |
| El token temporal usado en las pruebas debe rotarse | el usuario lo rota; Valet sólo guarda el definitivo |
| Cambios de esquema entre v0.16.0 y versiones futuras | el cliente tolera campos desconocidos y ausentes (`Option` + `default`) |
| Dependencia de un servicio externo para una función del agente | fallback: el TODO local sigue vivo hasta la fase 2, y los errores son legibles |
| El servidor **ignora** `?workspace_id=` | no dependemos de él: escribimos sin `workspace_id` (espacio por defecto) y no filtramos al leer; si se necesitara, el filtro va en Valet |
| `doing`, `waiting`, `shared` y `personal` se escriben como etiquetas | `set_label` con grupo y valor; el fragmento de prompt lo explica y nunca se envían ids a pelo |
| **`PATCH` de `labels` reemplaza el array** (verificado) | la fusión la hace el servicio leyendo la tarea; jamás enviar sólo la etiqueta de estado |
| Etiqueta borrada deja ids huérfanos en las tareas | la app los oculta; la herramienta resuelve por nombre y no recrea a ciegas |
| Los nombres de etiqueta son convención, no contrato | constantes documentadas en el servicio y en el fragmento de prompt; cambiarlas es cambio de código, no de datos |

## 7. Decisiones cerradas

Ninguna queda abierta. Las cinco últimas las tomó el usuario el 2026-10-27 al revisar el change proposal `opentodo-integration`; las demás se fijaron al aprobar el diseño.

- **Vistas guardadas: no se crean** (decisión del usuario). `doing`, `waiting`, `shared` y `personal` son etiquetas, y OpenTodo ya tiene un grupo «Labels» en el lateral: las etiquetas aparecen solas y al pulsarlas se ve su lista. Una vista guardada encima de una etiqueta sería la misma lista por otra puerta, con un nombre que puede desincronizarse del que usa Valet. Valet no escribe nada en la cuenta salvo las cuatro etiquetas cuando las necesita.
- **No se filtra por espacio de trabajo**: se escribe siempre en el de por defecto y se lista todo. Aceptado (una tarea creada a mano en «Trabajo» aparece en los listados); filtrar sería una línea por `workspace_id` si algún día se decide.
- **`shared`/`personal` son informativas**: la etiqueta expresa el ámbito, no lo concede. Compartir de verdad sigue siendo invitar a alguien al proyecto.
- **Las tareas locales se migran** (hoy son 3) al cerrar la fase 2; no se descartan.
- **La vista `Next` queda fuera** del primer alcance: depende del método de trabajo y de una agrupación por contexto no documentada como algoritmo.
- **Doble backend con fallback**: con `opentodo_token` configurado la herramienta va a OpenTodo; sin token conserva el comportamiento local actual, de modo que desplegar no cambia nada hasta pegar el token.
- **Prioridad omitida → `4`** (que es `low` y el valor por defecto del servidor), escrita siempre explícita para que la lectura sea inequívoca.
- **Permisos**: `NoConfirm` en lecturas, `Notify` en escrituras y `ExplicitApproval` en `delete_task`, también con `purge`.
- **`opentodo_base_url` no es sensible** y viaja en `PUT /api/settings`; solo el token tiene endpoint dedicado.

## 8. Tareas

- [x] 1. Aprobar alcance y las decisiones de §7. — Diseño cerrado el 2026-10-27.
- [x] 2. Crear el change `opentodo-integration` con delta de spec (servicio, tool, ajustes, fragmento de prompt, UI). — 4 deltas: `tools/tasks`, `opentodo-service`, `opentodo-api`, `frontend`.
- [x] 3. RED — tests del cliente (errores, timeout, deserialización con campos ausentes), de las vistas calculadas y de `set_label` (que no borre etiquetas ajenas y que cada grupo sea excluyente).
- [x] 4. GREEN — servicio, ajustes sensibles, endpoints de token/estado/sonda, migración, herramienta y fragmento de prompt.
- [x] 5. GREEN — octava sub-pestaña de Integraciones.
- [x] 6. REFACTOR — `cargo clippy -- -D warnings`, `cargo fmt --check`, `npx tsc --noEmit`, `npm run lint`. — 1947 tests backend y 644 frontend en verde.
- [ ] 7. Archivar el change y verificar en producción con turnos reales. — Archivado el 2026-10-27 y fusionado en `development` (PR #265, `99383c7`). La verificación contra la instancia real (tarea 9.3 de `tasks.md` del change archivado) queda **aplazada al despliegue** por decisión del usuario: el token ya está rotado y se pegará en Ajustes → Integraciones → OpenTodo para verificarlo desde la UI.
- [ ] 8. **Fase 2** — Phase 0: spec de caracterización de `tasks` local.
- [ ] 9. **Fase 2** — Change `remove-local-tasks`: volcado de datos, retirada de tabla/API/tool/vista y enlace a OpenTodo.
- [ ] 10. **Fase 2** — Archivar y release.
