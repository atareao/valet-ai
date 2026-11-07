# oximeal-service Specification

## Purpose
Define el cliente Rust de la Agent API de OxiMeal: la resolución de su
configuración en cada llamada (URL base, issuer, `client_id`/`client_secret`,
con respaldo de entorno), la obtención de un token M2M por `client_credentials`
contra Pocket ID con caché en memoria y renovación anticipada, y las seis
operaciones contra la Agent API: `recent_recipes`, `search_recipes`,
`set_weekly_plan`, `add_shopping_items`, `delete_completed` y `create_recipe`.
El secreto nunca aparece en `Debug` ni en los errores, y sin credenciales se
devuelve `NotConfigured` sin tocar la red.

## Requirements

### Requirement: El servicio `Oximeal` SHALL resolver su configuración en cada llamada y NO exponer nunca el secreto

El servicio `Oximeal` SHALL resolver en **cada** llamada, desde ajustes y entorno, `oximeal_base_url` (default `https://oximeal.territoriolinux.es`), `oximeal_resource` (default `<base_url>/api`, barra final recortada), `oximeal_issuer_url` (vacío ⇒ `AUTH_ISSUER_URL` de Valet), `oximeal_client_id` y `oximeal_client_secret`. SHALL redactar el secreto en `Debug` y en todo error, y sin credenciales SHALL devolver `NotConfigured` **sin tocar la red**.

#### Scenario: El secreto nunca aparece en el Debug
- **Given** una configuración con `client_secret = "SECRET"`
- **When** se formatea la configuración con `Debug`
- **Then** el resultado NO contiene `SECRET`

#### Scenario: Sin credenciales no se toca la red
- **Given** ni `oximeal_client_id` ni `oximeal_client_secret` (ni en ajustes ni en el entorno)
- **When** se construye el cliente con `from_settings`
- **Then** se devuelve `NotConfigured`
- **And** no se envía ninguna petición HTTP

#### Scenario: La URL base por defecto
- **Given** `oximeal_base_url` sin definir
- **When** se resuelve la configuración
- **Then** la URL base vale `https://oximeal.territoriolinux.es`

#### Scenario: El resource por defecto deriva de la URL base
- **Given** `oximeal_resource` sin definir (ni en ajustes ni en el entorno) y `oximeal_base_url` sin definir
- **When** se resuelve la configuración
- **Then** el `resource` vale `https://oximeal.territoriolinux.es/api`

#### Scenario: Un resource sin barra final
- **Given** `oximeal_base_url` = `https://oximeal.territoriolinux.es/` y `oximeal_resource` sin definir
- **When** se resuelve la configuración
- **Then** el `resource` vale `https://oximeal.territoriolinux.es/api`
- **And** NO termina en `//api`

#### Scenario: El resource explícito gana a la derivación
- **Given** `oximeal_resource` = `https://otra.api/api` y `oximeal_base_url` = `https://oximeal.territoriolinux.es`
- **When** se resuelve la configuración
- **Then** el `resource` vale `https://otra.api/api`

### Requirement: El servicio SHALL obtener un token M2M por `client_credentials` y cachearlo

Con `client_id` y `client_secret` presentes, el servicio SHALL descubrir el `token_endpoint` desde `<issuer>/.well-known/openid-configuration` (cacheando el descubrimiento) y SHALL solicitar el token con `grant_type=client_credentials`, `client_id`, `client_secret`, `resource=<resource resuelto>` y `scope=agent:read agent:plan agent:shopping agent:write` (`Content-Type: application/x-www-form-urlencoded`). El token SHALL cachearse en memoria de proceso y SHALL reutilizarse mientras no le falten menos de 30 segundos para expirar; a partir de ahí SHALL solicitarse uno nuevo. Un `401`/`403` de la Agent API SHALL invalidar la caché y traducirse a `Unauthorized`. Un fallo al obtener el token (el IdP no responde o responde con error) SHALL traducirse a `TokenEndpoint` sin filtrar credenciales.

#### Scenario: La petición de token lleva `resource`
- **Given** un `resource` resuelto de `https://oximeal.territoriolinux.es/api`
- **When** se solicita un token M2M por `client_credentials`
- **Then** el cuerpo de la petición incluye el par `resource` con el valor `https://oximeal.territoriolinux.es/api`
- **And** conserva `grant_type=client_credentials`, `client_id`, `client_secret` y el `scope`

#### Scenario: El token se cachea y se reutiliza
- **Given** un token obtenido con `expires_in = 3600`
- **When** se hacen dos operaciones seguidas
- **Then** el `token_endpoint` recibe **una sola** petición `client_credentials`
- **And** ambas operaciones llevan el mismo `Authorization: Bearer`

#### Scenario: El token expirado se renueva
- **Given** un token cuya expiración está a menos de 30 segundos
- **When** se hace una operación
- **Then** se solicita un token nuevo antes de llamar a la Agent API

#### Scenario: Un 401 invalida la caché
- **Given** un token en caché y la Agent API respondiendo `401`
- **When** se hace una operación
- **Then** el resultado es `Unauthorized`
- **And** la caché queda invalidada

#### Scenario: El IdP caído se distingue de un token inválido
- **Given** un `token_endpoint` que no responde
- **When** se hace una operación
- **Then** el resultado es `TokenEndpoint`

### Requirement: El servicio SHALL exponer las seis operaciones de la Agent API

El servicio SHALL implementar exactamente estas operaciones, contra los endpoints de la Agent API de OxiMeal:

| Operación | Endpoint | Forma |
|---|---|---|
| `recent_recipes(days)` | `GET /api/agent/recent-recipes?days=<u32>` | `days` por defecto 7, acotado a `1..=365` |
| `search_recipes(query, category)` | `GET /api/agent/search-recipes?query=&category=` | devuelve como mucho 10 recetas |
| `set_weekly_plan(entries)` | `POST /api/agent/weekly-plan` | cuerpo `{"entries":[…]}` con `date`, `meal_type`, `recipe_id`, `role`, `servings` |
| `add_shopping_items(items)` | `POST /api/agent/shopping-items` | cuerpo `{"items":[…]}` con `name`, `amount`, `unit`, `category?` |
| `delete_completed()` | `DELETE /api/agent/shopping/completed` | — |
| `create_recipe(input)` | `POST /api/agent/recipes` | cuerpo `RecipeInput` (`name`, `category`, `ingredients` obligatorios) |

Un `400` del servidor SHALL traducirse a `InvalidInput` conservando el mensaje del servidor (que nunca contiene credenciales); un `404` SHALL traducirse a `NotFound`; un `5xx` o un fallo de red SHALL traducirse a `Upstream`.

#### Scenario: La búsqueda de recetas usa query y categoría
- **Given** una llamada `search_recipes("pasta", "main")`
- **When** se ejecuta
- **Then** la petición va a `GET /api/agent/search-recipes` con `query=pasta` y `category=main`
- **And** la respuesta se devuelve como la lista de recetas recibida

#### Scenario: El plan semanal envía entradas estructuradas
- **Given** una llamada `set_weekly_plan` con una entrada `{date:"2026-11-09", meal_type:"dinner", recipe_id:"abc", role:"main", servings:4}`
- **When** se ejecuta
- **Then** el cuerpo enviado a `POST /api/agent/weekly-plan` es `{"entries":[{"date":"2026-11-09","meal_type":"dinner","recipe_id":"abc","role":"main","servings":4}]}`
- **And** la respuesta incluye `created` y `shopping_list`

#### Scenario: Un 400 del servidor conserva su mensaje
- **Given** la Agent API respondiendo `400` con `{"error":"categoría inválida: foo"}`
- **When** se ejecuta `create_recipe` con esa categoría
- **Then** el error es `InvalidInput` con el mensaje del servidor
- **And** el secreto NO aparece en el error
