# Design — opentodo-integration

## 1. Contrato de la herramienta `tasks`

`operation` (obligatorio):
`list_tasks | search_tasks | add_task | update_task | complete_task | delete_task | set_label`.

| Operación | Argumentos | Permiso |
|---|---|---|
| `list_tasks` | `view` (def. `all`), `project`, `limit` (def. 20, 1..=50) | `NoConfirm` |
| `search_tasks` | `query`, `limit` (def. 20, 1..=50) | `NoConfirm` |
| `add_task` | `title` (obligatorio), `description`, `project`, `priority` (def. `low`), `due` | `Notify` |
| `update_task` | `id` (obligatorio) + cualquiera de los anteriores, `start` | `Notify` |
| `complete_task` | `id` | `Notify` |
| `delete_task` | `id`, `purge` (def. `false`) | `ExplicitApproval` |
| `set_label` | `id`, `group` (`state`\|`scope`), `value` (`doing`\|`waiting`\|`shared`\|`personal`\|`null`) | `Notify` |

`view`: `all | inbox | today | upcoming | anytime | someday | doing | waiting`.

`due` acepta `YYYY-MM-DD`, `YYYY-MM-DDTHH:MM` o una expresión natural («el viernes a las 10»). Si
no es ISO, el servicio llama a `POST /api/v1/quick-add/parse` y usa su `due`. La **prioridad nunca**
se toma de ese parseo (no la reconoce): se escribe siempre explícita, y `low` → `4` es el valor con
el que el servidor nace sus tareas.

`list_tasks` y `search_tasks` devuelven:

```json
{"count": 42, "tasks": [{"id":"…","title":"…","project":"Personal","due":"2026-10-30","priority":"high","start":null,"labels":["doing"]}],
 "summary": {"by_view": {"inbox":3,"today":5,"upcoming":2,"anytime":8,"someday":1,"doing":2,"waiting":1}, "by_project": {"Personal":12,"(sin proyecto)":30}}}
```

## 2. Vistas (recalculadas en Rust, `completed_at == null` salvo donde se diga)

| Vista | Predicado |
|---|---|
| `inbox` | `project_id == null` |
| `today` | `due.date <= hoy` **o** `deadline <= hoy` |
| `upcoming` | `due.date > hoy`, más las sin `due` con `deadline` por delante |
| `anytime` | sin `due`, sin `deadline` y sin `start` |
| `someday` | `start == "someday"` |
| `doing` | lleva la etiqueta `doing` |
| `waiting` | lleva `waiting`, **o** `blocked_by` no vacío, **o** `assignee_id` distinto del mío |
| `all` | todo lo no completado |

`next` queda fuera del alcance.

## 3. Etiquetas

Constantes del servicio: `STATE_LABELS = ["doing","waiting"]`, `SCOPE_LABELS = ["shared","personal"]`.
No llevan `@` (decisión del usuario: así no se tratan como contextos en la vista Next).

`set_label(id, group, value)`:

1. lee la tarea (`GET /api/v1/tasks` filtrando por id en memoria; el servidor no tiene `GET /{id}`
   verificado, así que se usa el listado y se busca el id);
2. resuelve los ids de las etiquetas del grupo desde `GET /api/v1/labels` (coincidencia por nombre
   exacto, en el espacio por defecto);
3. si la etiqueta del grupo no existe, la crea con `POST /api/v1/labels` (sin `workspace_id` = espacio
   por defecto) y **avisa** en el resultado;
4. compone la lista final: las etiquetas actuales **menos** las del grupo, **más** la nueva si `value`
   no es `null`;
5. envía `PATCH /api/v1/tasks/{id}` con el array completo.

Reglas derivadas: nunca se envía un array incompleto; una etiqueta homónima creada por el usuario se
**adopta** en vez de duplicarse; una etiqueta que el usuario borró **no** se recrea en cada turno (se
informa en el mensaje).

## 4. Mapeo de campos

| Concepto | OpenTodo |
|---|---|
| título / descripción | `title` / `description` |
| bandeja de entrada | `project_id: null` |
| algún día | `start: "someday"` |
| completada | `completed_at` (ms Unix) |
| prioridad | `priority` 1..4 — `critical`→1, `high`→2, `medium`→3, `low`→4 |
| fecha límite | `due: {date, time?, tz?}` |
| proyecto | `project_id` (se resuelve o se crea por nombre) |
| estado / ámbito | etiquetas `doing`/`waiting` y `shared`/`personal` |

## 5. Cliente HTTP

- Base de operaciones: `GET /api/v1/me`, `GET|POST /api/v1/tasks`, `PATCH|DELETE /api/v1/tasks/{id}`,
  `DELETE /api/v1/tasks/{id}/purge`, `GET|POST /api/v1/labels`, `GET|POST /api/v1/projects`,
  `POST /api/v1/quick-add/parse`.
- Cabecera `Authorization: Bearer <token>`; `User-Agent: valet/1.0`; timeout 10 s.
- Toda **escritura** (`POST /tasks`, `PATCH /tasks/{id}`, `DELETE /tasks/{id}`,
  `DELETE /tasks/{id}/purge`, `POST /labels`) lleva una cabecera `Idempotency-Key` con un
  **UUID nuevo por petición**: sin ella un `409` no significaría nada y el aviso «ya aplicado»
  sería una mentira.
- Traducción de errores: red/5xx → `Upstream`; `401`/`403` → `Unauthorized`; `404` → `NotFound`;
  `400` → `InvalidInput` (con el mensaje del servidor, que no contiene el token); `409` de la
  `Idempotency-Key` que toda escritura envía → se trata como «ya aplicado», **no** se reintenta y
  la herramienta lo refleja en su mensaje («… (ya estaba aplicada)»).

## 6. Contrato HTTP de Valet

- `GET /api/opentodo/status` → `200 {"configured": bool, "base_url": "…"}`. Nunca el token.
- `PUT /api/opentodo/token` con `{"token": "…"}` → `204`; vacío o ausente → `400`.
- `GET /api/opentodo/check` → **siempre** `200 {"ok": bool, "detail": "…"}`, con un sondeo **real**
  (`GET /api/v1/me`) y **nunca** el token: con un token válido `{true, "conexión correcta"}`; sin
  token `{false, "no hay token de OpenTodo configurado"}`; `401`/`403` `{false, "el token no es
  válido o ha caducado"}`; red o `5xx` `{false, "OpenTodo no responde"}`. Es una sonda que informa
  de un estado, no un error de la API de Valet: por eso el código es `200` también cuando `ok` es
  `false`.

## 7. Alternativas descartadas

- **Doble herramienta (`tasks` local + `opentodo`)**: parte el dominio en dos skills y obliga al
  router a distinguir dos cosas idénticas.
- **Filtrar por espacio en `?workspace_id=`**: el servidor lo ignora; filtrar exigiría traer todo
  igualmente.
- **Etiquetar `inbox`/`someday`/`done`**: dos fuentes de verdad con los campos que la app escribe sola.
- **`doing` con `due` ficticio para que salga en Hoy**: corrompe la vista Hoy del usuario.
