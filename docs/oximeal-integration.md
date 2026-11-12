# Integración de OxiMeal

## Qué es

OxiMeal es el gestor *self-hosted* de recetas, menús semanales y listas de la
compra (repo hermano en `/data/rust/oximeal`, Axum + SQLite). Es *headless* y
*API-first*: no tiene orquestador ni chat propio, sino que delega la
conversación en **Valet** a través de su **Agent API** bajo `/api/agent`.

Esta feature da al agente de Valet una **herramienta nativa** (`oximeal`) que
habla con esa Agent API para poder leer el recetario, planificar la semana y
gestionar la lista de la compra. El agente resuelve las credenciales en cada
llamada y obtiene un token **M2M** contra el mismo IdP (PocketID) que ya usa
Valet.

OxiMeal **no se modifica** desde aquí: solo se consume.

## Ficheros implicados

| Fichero | Rol |
|---|---|
| `src/tools/oximeal.rs` | Herramienta multioperación `oximeal` (validación de argumentos, permisos por operación, despacho a la Agent API). |
| `src/services/oximeal.rs` | Cliente HTTP de la Agent API: resolución de config, token M2M cacheado, seis operaciones y traducción de errores. |
| `src/orchestrator/skills.rs` | Skill `menus` (`tools: &["oximeal"]`, `prompt_key: "SKILL_MENUS_PROMPT"`, umbral `0.10`). |
| `migrations/20261107000001_oximeal.sql` | Siembra `oximeal_base_url`, `oximeal_issuer_url`, `oximeal_client_id` y el fragmento `SKILL_MENUS_PROMPT`. |
| `src/handlers/oximeal.rs` | Handlers HTTP de integración: `status`, `check` y alta del secreto. |
| `src/routes/oximeal.rs` | Rutas `/api/oximeal/*`. |
| `src/lib.rs` | Registra la herramienta (`registry.register`) y monta las rutas. |
| `frontend/src/components/OximealSection.tsx` | Sub-pestaña «OxiMeal» dentro de Ajustes → Integraciones. |
| `frontend/src/hooks/useOximeal.ts` | Hook con el estado de la integración y las acciones. |
| `frontend/src/api/oximeal.ts` | Cliente frontend de `/api/oximeal/*`. |

## Cómo funciona

Flujo de **una llamada** del agente, por ejemplo `search_recipes`:

1. **Selección de skill.** El router de skills marca `menus` como activa
   (umbral `0.10`) y expone la herramienta `oximeal` al modelo.
2. **Validación de argumentos.** `src/tools/oximeal.rs` valida `operation` y sus
   parámetros **antes** de tocar la red. Un argumento inválido se rechaza con
   `InvalidArguments` y no envía ninguna petición.
3. **Permiso.** Según la operación, la herramienta pide `NoConfirm`, `Notify` o
   `ExplicitApproval` (ver tabla más abajo).
4. **Resolución de configuración.** `Oximeal::resolve_config` lee los ajustes
   `oximeal_*` de la base de datos y, si faltan o están vacíos, cae a las
   variables de entorno `OXIMEAL_*`. Sin `client_id` ni `client_secret` devuelve
   `NotConfigured` **sin tocar la red**.
5. **Token M2M.** El cliente descubre el `token_endpoint` en
   `{issuer}/.well-known/openid-configuration` (cacheado por emisor) y pide un
   token por `grant_type=client_credentials` con los scopes
   `agent:read agent:plan agent:shopping agent:write`. El token se guarda en una
   caché **de proceso** con margen de seguridad de 30 s; ante un `401`/`403` se
   invalida la caché y se reintenta **una** vez. El candado *single-flight* por
   emisor garantiza que, con la caché fría, el IdP reciba **una sola** petición
   de token aunque haya llamadas concurrentes.
6. **Llamada a la Agent API.** `send_authed` envía la petición con
   `Authorization: Bearer <token>` al endpoint correspondiente de OxiMeal.
7. **Traducción de la respuesta.** El cuerpo se mapea a DTOs tipados
   (`RecentRecipe`, `RecipeSummary`, `WeeklyPlanResult`, …). Los estados HTTP se
   traducen a `OximealError` (`400 → InvalidInput`, `401/403 → Unauthorized`,
   `404 → NotFound`, `5xx → Upstream`), que la herramienta convierte en mensajes
   accionables en español.

Operaciones de la herramienta y su permiso:

| `operation` | Endpoint de la Agent API | Permiso |
|---|---|---|
| `recent_recipes` | `GET /api/agent/recent-recipes?days=` | `NoConfirm` |
| `search_recipes` | `GET /api/agent/search-recipes?query=&category=` | `NoConfirm` |
| `plan_week` | `POST /api/agent/weekly-plan` (`{"entries":[…]}`) | `ExplicitApproval` |
| `add_shopping_items` | `POST /api/agent/shopping-items` | `Notify` |
| `clear_purchased` | `DELETE /api/agent/shopping/completed` | `ExplicitApproval` |
| `create_recipe` | `POST /api/agent/recipes` | `Notify` |

## Variables/config

La resolución es siempre **ajuste → variable de entorno → valor por defecto**
(este último solo para la URL base). La config se resuelve **en cada llamada**,
de modo que editar los ajustes surte efecto sin reiniciar.

| Ajuste (`settings`) | Variable de entorno | Defecto | Secreto |
|---|---|---|---|
| `oximeal_base_url` | `OXIMEAL_BASE_URL` | `https://oximeal.territoriolinux.es` | No |
| `oximeal_issuer_url` | `OXIMEAL_ISSUER_URL` | vacío ⇒ cae a `AUTH_ISSUER_URL` | No |
| `oximeal_client_id` | `OXIMEAL_CLIENT_ID` | vacío | No |
| `oximeal_client_secret` | `OXIMEAL_CLIENT_SECRET` | vacío | **Sí** |

`oximeal_client_secret` figura en `SENSITIVE_KEYS`
(`src/db/repos/settings.rs`): `GET /api/settings` lo **omite** y
`PUT /api/settings` lo **ignora**. Su única puerta de escritura es
`PUT /api/oximeal/secret`.

### Endpoints de integración de Valet

| Método | Ruta | Qué hace |
|---|---|---|
| GET | `/api/oximeal/status` | Estado sin el secreto: `{configured, base_url, issuer_url, has_client_id}` |
| GET | `/api/oximeal/check` | Sonda real (obtiene token y consulta `recent-recipes`). Siempre `200` con `{ok, detail}` |
| PUT | `/api/oximeal/secret` | Guarda el `client_secret` (`{"client_secret": "…"}`): `400` si va vacío, `204` si se guarda |

Todas exigen **sesión OIDC**. Ejemplos con `curl` (cookie de sesión):

```bash
# Estado de la configuración (nunca devuelve el secreto).
curl -s https://valet.example.com/api/oximeal/status \
  -H 'Cookie: valet_session=<tu-sesión>'

# Sonda real contra OxiMeal y su IdP. Responde 200 aunque falle:
# el veredicto viaja en `ok`/`detail`.
curl -s https://valet.example.com/api/oximeal/check \
  -H 'Cookie: valet_session=<tu-sesión>'

# Alta del client_secret por su endpoint dedicado (204 sin cuerpo).
curl -s -o /dev/null -w '%{http_code}\n' \
  -X PUT https://valet.example.com/api/oximeal/secret \
  -H 'Cookie: valet_session=<tu-sesión>' \
  -H 'Content-Type: application/json' \
  -d '{"client_secret":"<el-secreto-M2M>"}'
```

## Cómo habilitar/desplegar

1. **Crear el cliente M2M en PocketID.** OxiMeal solo admite OIDC M2M
   (`grant_type=client_credentials`), así que hace falta un cliente de tipo
   *machine-to-machine* en el mismo IdP que Valet. Asígnale los scopes
   `agent:read agent:plan agent:shopping agent:write`.
2. **Introducir el `client_id`.** En Valet: Ajustes → Integraciones → OxiMeal,
   en el campo *Client ID* (es un ajuste normal, se guarda con
   `PUT /api/settings`).
3. **Introducir el `client_secret`.** En el mismo panel, en el campo
   *Client Secret*. Es una clave sensible y se guarda por su endpoint dedicado
   `PUT /api/oximeal/secret`; el campo arranca **siempre vacío** y nunca relee
   el valor guardado.
4. **Comprobar la conexión.** El botón «Probar conexión» llama a
   `GET /api/oximeal/check`, que devuelve un veredicto legible.
5. **Ajustar la URL base si procede.** `oximeal_base_url` ya trae el valor por
   defecto; solo hay que cambiarla si OxiMeal no vive en
   `https://oximeal.territoriolinux.es`. El campo *Issuer URL* puede dejarse
   vacío para reutilizar `AUTH_ISSUER_URL`.

Alternativamente, todo puede pre-configurarse por entorno: ver el bloque
`OXIMEAL_*` de `.env.example`.

## Decisiones

- **Herramienta única, multioperación.** `oximeal` agrupa las seis operaciones
  bajo un parámetro `operation` (molde de `zennotes`/`opentodo`), en lugar de
  repartirlas en varias herramientas. La validación ocurre antes de resolver la
  config y de tocar la red.
- **Skill `menus` dedicada.** El recetario y los menús son un dominio propio, así
  que la herramienta vive en su propia skill (`menus`, umbral `0.10`) y no se
  reparte con otras. `oximeal` es enrutable, no pertenece al core.
- **El `client_secret` es sensible.** Nunca sale por `GET /api/settings`, no se
  guarda por `PUT /api/settings`, y la rotación (`PUT /api/oximeal/secret`)
  invalida la caché del token para que no se siga sirviendo uno obtenido con las
  credenciales viejas. Ningún mensaje de error filtra credenciales.
- **`role` sin `dessert`.** El *rol* de una entrada del plan solo admite
  `main | salad | side` (OxiMeal rechaza `dessert` con `400`), mientras que la
  *categoría* de receta sí admite `main | salad | side | dessert | breakfast`. La
  herramienta valida el rol localmente para que el error no viaje a OxiMeal.
- **Caché de token *single-flight*.** El token M2M y el `token_endpoint` se
  cachean por proceso con margen de 30 s; un candado por emisor evita que N
  llamadas concurrentes pidan N tokens. Cambiar el `client_id` o el emisor
  invalida el token cacheado.
- **Las etiquetas de receta se decodifican como objetos y se exponen como
  nombres.** OxiMeal (feature *recipe-tags*) envía `RecipeSummary.tags` como
  objetos `{id, name, slug, source}`; `TagRef` se queda con el `name` e ignora el
  resto, y la salida de la herramienta sigue siendo una lista de cadenas. Con
  `Vec<String>` toda búsqueda reventaba con `invalid type: map, expected a
  string`.
- **El detalle de un `4xx` prefiere `message` a `error`.** OxiMeal usa `error`
  como código máquina (`validation`, `not_found`) y deja la explicación humana en
  `message`; el orden de extracción es `message`, `error`, `detail`, `title`. El
  propio cuerpo recortado a 300 caracteres se usa cuando el cuerpo no es JSON
  legible **o** cuando sí lo es pero ninguna de esas cuatro claves es una cadena.
- **El `recipe_id` de `plan_week` no se inventa.** El esquema que se envía al
  modelo lo dice explícitamente, tanto en la descripción de `entries` como en la
  de la propia propiedad `recipe_id`: el identificador sale de `search_recipes` o
  del `id` que devuelve `create_recipe`, nunca de un nombre de receta. **No** se
  pre-valida con una llamada extra a `search_recipes` (una petición de más por
  cada entrada y una fuente de fallo añadida): la corrección llega por el `400`
  del servidor, ahora legible, dentro del mismo turno.
- **`GET /api/oximeal/check` es la sonda real.** Obtiene un token y consulta
  `recent-recipes`; responde siempre `200` con `{ok, detail}` porque que OxiMeal
  (o su IdP) no responda es precisamente lo que se averigua. El `detail` separa
  estructuralmente las cuatro averías: configuración incompleta, el IdP no
  entregó token, el token fue rechazado y OxiMeal no responde.

## Follow-ups

- **E2E pendiente: OxiMeal no está desplegado en `co1`.** No se ha podido
  verificar el ciclo completo contra una instancia real (crear el cliente M2M en
  PocketID, pegar las credenciales, «Probar conexión» y una operación real de la
  herramienta). La verificación de extremo a extremo queda por hacer cuando
  OxiMeal esté accesible.
- **El `example` de `curl` del README de OxiMeal está desactualizado.**
  `POST /api/agent/weekly-plan` espera hoy `{"entries": […]}`, no el formato
  plano que muestra el README del repo de OxiMeal.
