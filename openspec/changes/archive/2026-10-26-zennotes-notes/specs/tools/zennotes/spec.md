## Purpose

La herramienta `zennotes` de Valet: lee y escribe las notas del vault de ZenNotes del usuario contra la API HTTP de su servidor autoalojado (Bearer token), con lecturas sin confirmación, escrituras aditivas con aviso e irreversibles con aprobación explícita, y recorte obligatorio del listado.

## ADDED Requirements

### Requirement: `zennotes` SHALL despachar por `operation` y habilitar las operaciones de lectura del vault

La herramienta `zennotes` SHALL exponer un parámetro `operation` con el enum
`["list","search","read","create","append","replace","write","move","archive","unarchive","rename","trash","restore","delete"]`
y `required: ["operation"]`. `list` SHALL consultar `GET /api/notes` y SHALL aceptar los filtros
opcionales `folder` (`inbox`/`quick`/`archive`), `subpath`, `tag` y `limit`. `read` SHALL consultar
`GET /api/notes/read?path=<rel>`. `search` SHALL consultar `GET /api/search/text?q=<q>&backend=auto`
en modo `text`, y en modo `title`/`tag` SHALL filtrar localmente sobre los metadatos del listado.
`list`, `search` y `read` SHALL declarar `Permission::NoConfirm` y SHALL NO modificar ninguna nota.

#### Scenario: list devuelve las notas sin tocar el vault
- **Given** un vault con notas
- **When** se ejecuta `zennotes` con `{"operation":"list"}`
- **Then** se devuelven las notas consultadas
- **And** ninguna nota cambia

#### Scenario: read trae el cuerpo de una nota
- **Given** una nota existente con `path` conocido
- **When** se ejecuta `zennotes` con `{"operation":"read","path":"…"}`
- **Then** se devuelve su `body`
- **And** el permiso de la operación es `NoConfirm`

### Requirement: `zennotes` SHALL recortar el listado y el cuerpo para no volcar el vault al prompt

`GET /api/notes` devuelve **6.273** notas (≈3,3 MB), así que `list` SHALL devolver un objeto con
`count` (total tras filtro) y `notes` (hasta `limit`), y cada nota SHALL incluir **solo** `path`,
`title`, `folder`, `tags` y `updatedAt`. El `limit` de `list` SHALL ser opcional, valer `20` por
defecto y estar acotado a `1..=200`; fuera de rango SHALL devolver `Err(ToolError::InvalidArguments)`
**sin salir a la red**. `read` SHALL devolver el `body` recortado a `20000` caracteres e indicar si
hubo recorte. `search` SHALL acotar su resultado por `limit` (por defecto `20`).

#### Scenario: El listado va acotado y con campos mínimos
- **Given** un vault con más notas que el `limit` pedido
- **When** se ejecuta `zennotes` con `{"operation":"list","limit":5}`
- **Then** se devuelven como mucho cinco notas
- **And** cada nota trae `path`, `title`, `folder`, `tags` y `updatedAt`
- **And** no incluye el `body`

#### Scenario: Un limit fuera de rango es un argumento inválido
- **Given** una llamada con `limit` igual a `0` o a `500`
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición al servidor

#### Scenario: El cuerpo de read se recorta a 20000 caracteres
- **Given** una nota cuyo `body` supera los `20000` caracteres
- **When** se ejecuta `zennotes` con `{"operation":"read","path":"…"}`
- **Then** el `body` devuelto mide como mucho `20000` caracteres
- **And** se indica que hubo recorte

### Requirement: `zennotes` SHALL escribir en el vault en `inbox/Valet` por defecto y los permisos por operación

`create` SHALL aceptar `title` (obligatorio) y, opcionalmente, `body`, `folder` (por defecto `inbox`) y
`subpath` (por defecto el ajuste `zennotes_folder`, cuyo valor por defecto es `Valet`). Como en la API
del servidor `folder` es un enum **restringido** a `inbox`/`quick`/`archive` y `Valet` es una
**subcarpeta**, la nota SHALL nacer en `inbox/Valet/`; un `folder` fuera del enum SHALL devolver
`Err(ToolError::InvalidArguments)` **sin salir a la red**. Como la ruta `create` del servidor **no
acepta contenido**, crear con cuerpo SHALL ser create + write (dos viajes).
`append` y `replace` SHALL implementarse con leer→modificar el `body`→escribir. Los permisos SHALL ser
`Notify` para `create`, `append`, `replace`, `move`, `archive`, `unarchive`, `rename` y `restore`, y
`ExplicitApproval` para `write` (sobrescritura), `trash` y `delete`. La operación `empty-trash` SHALL
NO exponerse.

#### Scenario: create sin destino crea la nota bajo el subpath Valet dentro de inbox
- **Given** el ajuste `zennotes_folder` con su valor por defecto `Valet`
- **When** se ejecuta `zennotes` con `{"operation":"create","title":"Bitácora"}`
- **Then** la nota se envía al servidor con `folder` `inbox` y `subpath` `Valet`
- **And** la nota queda bajo el subpath `Valet` dentro de `inbox` (`inbox/Valet/`)
- **And** el permiso de la operación es `Notify`

#### Scenario: un folder fuera del enum es un argumento inválido
- **Given** una llamada con `{"operation":"create","title":"X","folder":"Valet"}`
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición al servidor

#### Scenario: create con cuerpo hace dos viajes
- **Given** una llamada con `{"operation":"create","title":"X","body":"# Hola"}`
- **When** se ejecuta
- **Then** primero se crea la nota y después se escribe su `body`

#### Scenario: Los permisos por operación
- **Given** la herramienta `zennotes` registrada
- **When** se consulta su permiso con `{"operation":"write"}` y con `{"operation":"read"}`
- **Then** `write` es `ExplicitApproval`
- **And** `read` es `NoConfirm`
- **And** `trash` y `delete` son `ExplicitApproval`

### Requirement: El servicio `Zennotes` SHALL resolver su configuración en cada llamada y NO exponer nunca el token

El cliente SHALL resolver `zennotes_base_url`, `zennotes_token` y `zennotes_folder` desde `settings`
en cada ejecución, con `ZENNOTES_BASE_URL`/`ZENNOTES_TOKEN`/`ZENNOTES_FOLDER` del entorno como
alternativa y, **solo** para la URL, el valor por defecto `https://zennotes.territoriolinux.es`. La
falta de token SHALL producir un error `MissingCredentials` **distinto** de `Unauthorized` (token
inválido). Ni el `Debug` de la configuración ni el mensaje de ningún `ZennotesError` SHALL contener el
token.

#### Scenario: Sin token el error es MissingCredentials
- **Given** sin token en `settings` ni en el entorno
- **When** se intenta una operación
- **Then** el error es `MissingCredentials`
- **And** el mensaje indica dónde configurarlo

#### Scenario: El token nunca aparece en un error ni en un Debug
- **Given** una configuración con token
- **When** se formatea su `Debug` y se produce un error `Http`
- **Then** ni el `Debug` ni el mensaje contienen el valor del token
