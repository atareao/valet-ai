# Parámetro `resource` (RFC 8707) en el token M2M de OxiMeal

## Qué es

El parámetro **`resource`** de una petición OAuth2 `client_credentials` (definido
en **RFC 8707**, *Resource Indicators*) declara **para qué API** se pide el token.
Pocket ID lo exige para resolver a qué recurso pertenece un *scope*
personalizado: sin él, al pedir un token con los scopes `agent:*` responde
`invalid_scope` — «*The scope 'agent:read' is not available for the requested
resource.*» — porque no sabe a qué audiencia asignarlos.

Esta feature arregla precisamente eso: la petición M2M que hace Valet hacia
Pocket ID ahora envía `resource`. Sin él, la conexión Valet↔OxiMeal estaba
**rota**: Valet mostraba el fallo como «el IdP no entregó token», un mensaje
opaco que ocultaba el error real.

Con `resource=https://oximeal.territoriolinux.es/api`, Pocket ID emite un token
cuyo `aud` es `["https://oximeal.territoriolinux.es/api"]`, que es justo lo que
OxiMeal valida contra su `OIDC_API_AUDIENCE`. Sin `resource`, aunque Pocket ID
aceptara la petición, el `aud` del token sería el `client_id` y OxiMeal lo
rechazaría con `401`.

## Ficheros implicados

| Fichero | Rol |
|---|---|
| `src/services/oximeal.rs` | Resuelve `resource` (ajuste → entorno → derivado `<base_url>/api`) y lo envía en la petición `client_credentials` de `fetch_token`; el detalle del error viaja ya redactado. |
| `src/handlers/oximeal.rs` | `GET /api/oximeal/status` devuelve `resource`; `check_detail` pasa a `String` y expone el detalle redactado del IdP y de OxiMeal. |
| `src/tools/oximeal.rs` | Helper de test reutilizado por los casos de `resource`. |
| `frontend/src/types/index.ts` | Añade `resource` al tipo `OximealStatus`. |
| `frontend/src/components/OximealSection.tsx` | Campo no sensible «OxiMeal · Resource» y reflejo del `resource` resuelto en la línea de estado. |
| `frontend/src/components/SettingsDialog.tsx` | Registra `oximeal_resource` entre los ajustes no sensibles de la sección «OxiMeal». |

## Cómo funciona

1. **Resolución del `resource`.** Se resuelve en **cada llamada**, con la
   precedencia ajuste → variable de entorno → derivado:

   `oximeal_resource` → `OXIMEAL_RESOURCE` → `<base_url>/api` (recortando la
   barra final, para no producir `//api`). Para la configuración por defecto
   vale `https://oximeal.territoriolinux.es/api`.

2. **Petición de token.** Con `client_id` y `client_secret`, el cliente descubre
   el `token_endpoint` en `<issuer>/.well-known/openid-configuration` y hace
   `POST` con `grant_type=client_credentials`, `client_id`, `client_secret`,
   `scope=agent:read agent:plan agent:shopping agent:write` y
   **`resource=<resource resuelto>`**. El token se cachea en proceso con margen
   de 30 s (misma caché *single-flight* que ya existía).

3. **Detalle redactado en la sonda.** `check_detail` ha dejado de ocultar la
   causa y devuelve un `String` con el detalle redactado (nunca el secreto ni el
   token):

   - IdP sin entregar token → `el IdP no entregó un token M2M: <detalle>`;
   - OxiMeal caído o `5xx` → `OxiMeal no responde: <detalle>`;
   - sin credenciales → `configuración incompleta`;
   - token rechazado → `el token fue rechazado`;
   - todo correcto → `conexión correcta`.

   Ese `<detalle>` es lo que hizo visible el `invalid_scope` real de Pocket ID.

## Variables / configuración

La resolución es siempre **ajuste → variable de entorno → valor por defecto** y
se evalúa en cada llamada, de modo que editar los ajustes surte efecto sin
reiniciar.

| Ajuste (`settings`) | Variable de entorno | Defecto | Secreto |
|---|---|---|---|
| `oximeal_resource` | `OXIMEAL_RESOURCE` | derivado `<base_url>/api` | No |

`oximeal_resource` **no** es sensible: viaja en `GET`/`PUT /api/settings` como
`oximeal_base_url`, `oximeal_issuer_url` y `oximeal_client_id`.

> **Recordatorio:** el `resource` debe coincidir con el `OIDC_API_AUDIENCE` que
> OxiMeal valida. Con la configuración por defecto, ambos son
> `https://oximeal.territoriolinux.es/api`; si OxiMeal vive en otra URL, hay que
> alinear `oximeal_base_url` (o fijar `oximeal_resource` a mano) con su
> `OIDC_API_AUDIENCE`.

## Cómo habilitar / desplegar

1. **Desplegar la imagen nueva de Valet** (incluye el `resource` en la petición
   M2M y el detalle redactado en la sonda).
2. **En la UI**, Ajustes → Integraciones → OxiMeal: rellenar el `client_id` y el
   `client_secret` del cliente M2M (el secreto se guarda por su endpoint
   dedicado `PUT /api/oximeal/secret`; el campo arranca siempre vacío).
3. **Pulsar «Probar conexión»** (`GET /api/oximeal/check`). El veredicto
   esperado es `{ok:true, detail:"conexión correcta"}`. Si falla, el `detail`
   ahora muestra la causa real (por ejemplo, el error del IdP).

No hay migración nueva: `oximeal_resource` tiene default derivado y no necesita
siembra.

## Decisiones

- **Derivar vs exigir.** El `resource` tiene un default derivado de la URL base
  (`<base_url>/api`) en vez de exigir configurarlo siempre a mano. Así la
  configuración por defecto funciona sin ningún paso extra y, a la vez, puede
  sobreescribirse por ajuste o entorno si la API de OxiMeal no cuelga de `/api`.
- **Exponer el detalle redactado vs mensaje opaco.** La sonda muestra el
  detalle ya redactado del IdP y de OxiMeal. Un mensaje opaco («el IdP no entregó
  token») escondía el `invalid_scope` real y alargó el diagnóstico; el detalle
  redactado explica la causa sin filtrar nunca el secreto ni el token.
- **`resource` solo en M2M.** Es un parámetro de la petición
  `client_credentials`; no interviene en el OIDC de sesión de usuario de Valet ni
  en otras integraciones.

## Follow-ups

1. **`resolve_config` lee `base_url` dos veces** al resolver `resource` (una
   para el default derivado y otra para el valor final). Perf menor; se puede
   leer una sola vez.
2. **Normalización asimétrica del `resource` explícito:** el derivado recorta la
   barra final, pero un `oximeal_resource` fijado a mano no. Convendría
   normalizar ambos caminos por igual.
3. **El brazo `_` de `check_detail`** manda `NotFound`/`InvalidInput` a «OxiMeal
   no responde» sin el detalle. Revisar si merecen su propio mensaje.
4. **`redact` no sustituye el token M2M.** Como defensa en profundidad, la
   redacción podría cubrir también el propio `access_token`.
5. **Sin test del reset de `oximeal_resource`** en el formulario. Añadir un caso
   que verifique que vaciar el campo y guardar lo restablece.
