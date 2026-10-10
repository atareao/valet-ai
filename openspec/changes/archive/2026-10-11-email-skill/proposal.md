# Change: Skill `email` — correo (listar no leídos, leer un cuerpo, marcar como leído y enviar)

## Why

Valet no tiene ninguna vía para leer ni para escribir correo. Existe ya el servicio `apimail`,
desplegado en `https://apimail.territoriolinux.es`, cuya API HTTP protege todas sus rutas salvo la
sonda de salud con `Authorization: Bearer <API key>` y que resuelve las cuatro operaciones que hacen
falta: listar los mensajes **no leídos** de un buzón (`GET /api/messages?mailbox=INBOX&unseen=true`),
obtener el **cuerpo** de un mensaje (`GET /api/messages/{uid}/body`, que pide el cuerpo con
`BODY.PEEK` y por tanto **no** fija `\Seen`), **marcar como leído**
(`PATCH /api/messages/{uid}/flags` con `{"add":["\\Seen"]}`, idempotente) y **enviar** correo
(`POST /api/messages`).

El enrutador de skills ya sabe repartir capacidades por dominio: una skill nueva con sus herramientas
basta para que el modelo pueda consultar y responder el correo **solo** en los turnos en que haga
falta, sin engordar el prompt de todos los turnos.

**Limitación conocida y aceptada.** `POST /api/messages` **no admite `In-Reply-To` ni `References` ni
cabeceras propias**: su `MessageBuilder` escribe las que él decide. Por tanto responder es posible de
forma **funcional** —destinatario y asunto correctos, un solo paso— pero la respuesta **no quedará
enganchada en el hilo** del cliente del destinatario. Es una limitación de apimail, no de Valet; si
algún día se quiere hilo real, se añade allí.

## What Changes

- **Nueva skill `email`** en el catálogo cerrado (`Skill::Email`), la **novena**, con su pregunta, sus
  dos criterios, su umbral `0.10`, su clave de fragmento `SKILL_EMAIL_PROMPT` y su encabezado
  `# SKILL ACTIVA: EMAIL`. Habilitada por defecto.
- **Cuatro herramientas nuevas**, registradas en `build_tool_registry` y cubiertas por la skill
  `email`: `email_list_unread`, `email_get_body`, `email_mark_read` y `email_send`. Las tres primeras
  operan sobre `INBOX` y declaran `Permission::NoConfirm`; **`email_send` declara
  `Permission::ExplicitApproval`**, porque enviar es irreversible y sale del sistema.
- **Responder en un solo paso**: `email_send` acepta un `reply_to_uid` opcional con el que resuelve
  por su cuenta el destinatario —el remitente del envelope original— y el asunto —`Re: <original>`,
  sin duplicar el prefijo—, de modo que el modelo solo aporta el cuerpo.
- **Nueva capa de servicio `Apimail`** (`src/services/apimail.rs`) que resuelve la URL base y la API
  key —de `settings` primero (`apimail_base_url`, `apimail_api_key`), del entorno después
  (`APIMAIL_BASE_URL`, `APIMAIL_API_KEY`) y, solo para la URL, de un valor por defecto
  `https://apimail.territoriolinux.es`— y traduce los errores de la API a mensajes accionables.
- **Migración aditiva e idempotente** que siembra `ROUTER_SKILL_EMAIL_ENABLED`, `SKILL_EMAIL_PROMPT`,
  `apimail_base_url` y `apimail_api_key`, sin pisar valores existentes.
- **Pestaña «API Keys»** con dos campos nuevos: la URL base de apimail y su API key.

## Impact

### Specs modificadas

- `orchestrator/skill-router`: el catálogo cerrado pasa de ocho a **nueve** skills y cubre
  **veinticuatro** herramientas; la consulta por API devuelve nueve.
- `tools/registry`: el registro de producción incluye las cuatro herramientas `email_*`, y el
  requisito de aprobación explícita nombra `email_send`.
- `frontend`: la pestaña «API Keys» gana los campos de apimail.

### Specs nuevas

- `tools/email`: las cuatro herramientas y su contrato con la API de apimail.

### Código

- `src/orchestrator/skills.rs` (variante, catálogo y tests de integridad), `src/services/apimail.rs`
  (nuevo), `src/services/mod.rs`, `src/tools/email.rs` (nuevo), `src/tools/mod.rs`, `src/lib.rs`
  (registro), `frontend/src/components/SettingsDialog.tsx` (dos campos) y
  `frontend/src/test/SettingsDialog.test.tsx`. Una migración nueva en `migrations/` y su target de
  test `email-migration`. Las variables de entorno `APIMAIL_BASE_URL` y `APIMAIL_API_KEY` se leen
  directamente en el servicio (mismo precedente que Strava), sin tocar `src/config.rs`.

### Datos

- Migración nueva que siembra cuatro claves. No se reconstruye ninguna tabla.

### No-objetivos

- **No hay hilo real**: apimail no admite `In-Reply-To`/`References`; la respuesta lleva destinatario
  y `Re:` correctos, pero no se engancha al hilo del destinatario.
- **No se implementan adjuntos** (`attachments`), ni el borrado, el movimiento, la copia ni las
  banderas distintas de `\Seen`: fuera del alcance pedido.
- **No hay búsqueda de correos ya leídos**: la única lectura es el listado de no leídos.
- **No se toca `apimail`**: se consume su API tal y como está documentada.
- **No se generaliza el buzón**: las cuatro herramientas operan sobre `INBOX`.
- **No se expone el remitente (`from`)**: lo resuelve apimail con el usuario SMTP configurado.
- **No se toca `docker-compose.prod.yml`**: la configuración se hace desde la UI, con respaldo por
  variables de entorno a cargo del operador.
