# github — Delta de especificación (change github-integration)

## ADDED Requirements

### Requirement: El servicio `Github` SHALL resolver su configuración en cada llamada y NO exponer nunca el token

El servicio `Github` SHALL resolver en **cada** llamada, desde `settings`, el token
(`github_token`), el repositorio por defecto (`github_default_repo`, default `atareao/valet-ai`) y
los labels por defecto (`github_default_labels`, lista separada por comas). `settings` SHALL ser la
**única** fuente de configuración: no SHALL existir respaldo por variables de entorno. El token
SHALL viajar en la cabecera `Authorization: Bearer <token>` contra la URL **fija**
`https://api.github.com` (no configurable) y SHALL redactarse en `Debug` y en todo error. Sin
`github_token` SHALL devolver `NotConfigured` **sin tocar la red**.

**Contracts:**

```rust
pub struct GithubConfig {
    pub token: String,
    pub default_repo: String,
    pub default_labels: Vec<String>,
}

pub enum GithubError {
    NotConfigured,
    InvalidArguments(String),
    Unauthorized,
    NotFound,
    InvalidInput(String),
    RateLimited,
    Upstream(String),
}
```

#### Scenario: El token nunca aparece en el Debug

- **Given** una configuración con `token = "SECRET"`
- **When** se formatea la configuración con `Debug`
- **Then** el resultado NO contiene `SECRET`

#### Scenario: Sin token no se toca la red

- **Given** `github_token` vacío o ausente en `settings`
- **When** se construye el cliente con `resolve_config`
- **Then** se devuelve `NotConfigured`
- **And** no se envía ninguna petición HTTP

#### Scenario: La URL del API es fija

- **Given** el servicio `Github`
- **When** se resuelve su configuración
- **Then** la URL base es `https://api.github.com`
- **And** no existe ningún ajuste que la sobreescriba

#### Scenario: El repositorio por defecto se lee de los ajustes

- **Given** `github_default_repo = "atareao/valet-ai"` en `settings`
- **When** se resuelve la configuración
- **Then** `default_repo` vale `atareao/valet-ai`

#### Scenario: Los labels por defecto se parsean de la lista con comas

- **Given** `github_default_labels = "feature, valet"` en `settings`
- **When** se resuelve la configuración
- **Then** `default_labels` vale `["feature", "valet"]`
- **And** los espacios alrededor de cada label se recortan

### Requirement: El servicio `Github` SHALL crear el issue con `POST /repos/{repo}/issues` y traducir los errores sin filtrar el token

Con un token resuelto, `create_issue` SHALL emitir **una sola** petición `POST` a
`https://api.github.com/repos/{repo}/issues` con la cabecera `Authorization: Bearer <token>` y el
cuerpo `{"title": <string>, "body": <string?>, "labels": <[string]?>}`, enviando `body` y `labels`
**solo** cuando estén presentes. El único resultado de éxito SHALL ser `201 Created`. El servicio
SHALL NOT reintentar. Los estados SHALL traducirse a errores tipados, con un mensaje accionable en
español que **nunca** contiene el token:

- `401`/`403` → `Unauthorized`;
- `404` → `NotFound` (el repositorio no existe o el token no lo ve);
- `422` → `InvalidInput` con el mensaje de GitHub (p. ej. un label inexistente);
- `429` → `RateLimited`;
- `5xx` o fallo de red → `Upstream`.

#### Scenario: La petición lleva el Bearer y el cuerpo correcto

- **Given** las credenciales resueltas y `repo = "atareao/valet-ai"`
- **When** se crea un issue con `title`, `body` y `labels`
- **Then** la petición es `POST /repos/atareao/valet-ai/issues`
- **And** lleva `Authorization: Bearer <token>`
- **And** el cuerpo lleva `title`, `body` y `labels`

#### Scenario: Los campos opcionales se omiten cuando faltan

- **Given** una llamada a `create_issue` con solo `title`
- **When** se construye la petición
- **Then** el cuerpo lleva `title`
- **And** NO lleva `body` ni `labels`

#### Scenario: Solo `201` es éxito

- **Given** GitHub respondiendo `201` con el issue creado
- **When** se crea el issue
- **Then** la llamada devuelve el número y la URL del issue
- **And** un `200` u otro estado distinto de `201` NO se considera éxito

#### Scenario: Un token inválido se distingue de un repositorio ausente

- **Given** GitHub respondiendo `401`
- **When** se crea el issue
- **Then** se devuelve `Unauthorized`
- **And** con `404` se devuelve `NotFound`

#### Scenario: Un `422` explica el rechazo

- **Given** GitHub respondiendo `422` con un mensaje de validación
- **When** se crea el issue
- **Then** se devuelve `InvalidInput` con el mensaje de GitHub
- **And** el mensaje NO contiene el token

#### Scenario: El límite de peticiones se distingue de una caída

- **Given** GitHub respondiendo `429`
- **When** se crea el issue
- **Then** se devuelve `RateLimited`
- **And** con un `5xx` o un fallo de red se devuelve `Upstream`

#### Scenario: No se reintenta una escritura

- **Given** GitHub respondiendo `5xx`
- **When** se crea el issue
- **Then** se registra exactamente **una** petición
- **And** no se emite ninguna repetición

### Requirement: La herramienta `github` SHALL validar sus argumentos sin salir a la red

La herramienta `github` SHALL exponer la operación `create_issue` con `title` obligatorio y no vacío,
y los parámetros opcionales `body`, `repo` y `labels`. SHALL resolver el repositorio como `repo` (si
se pasa y es válido) o, en su defecto, `github_default_repo`, y los labels como `labels` (si se
pasan) o, en su defecto, `github_default_labels`. `repo` SHALL tener forma `owner/name` (exactamente
una barra, ambos tramos no vacíos). Un argumento inválido SHALL producir `InvalidArguments` con un
mensaje accionable **sin** emitir ninguna petición HTTP. Sin `repo` ni repositorio por defecto, la
herramienta SHALL fallar con un mensaje accionable **sin** salir a la red.

#### Scenario: Un título vacío se rechaza sin red

- **Given** una llamada con `title` vacío
- **When** se valida la llamada
- **Then** se devuelve `InvalidArguments`
- **And** no se envía ninguna petición HTTP

#### Scenario: Un repo mal formado se rechaza sin red

- **Given** una llamada con `repo = "no-es-un-repo"`
- **When** se valida la llamada
- **Then** se devuelve `InvalidArguments`
- **And** no se envía ninguna petición HTTP

#### Scenario: El parámetro repo gana al valor por defecto

- **Given** `github_default_repo = "atareao/valet-ai"` y una llamada con `repo = "otro/repo"`
- **When** se resuelve el destino
- **Then** el issue se crea en `otro/repo`

#### Scenario: Sin repo ni default falla sin tocar la red

- **Given** `github_default_repo` vacío y una llamada sin `repo`
- **When** se resuelve el destino
- **Then** la llamada falla con un mensaje accionable
- **And** no se envía ninguna petición HTTP

#### Scenario: Los labels del parámetro sustituyen a los del ajuste

- **Given** `github_default_labels = "valet"` y una llamada con `labels = ["feature"]`
- **When** se resuelve el conjunto de labels
- **Then** el issue se crea con `["feature"]`

### Requirement: La herramienta `github` SHALL declarar su permiso en función de la operación

La herramienta `github` SHALL declarar el permiso `Notify` para la operación `create_issue`: es una
escritura que avisa, pero no es destructiva ni exige aprobación explícita.

#### Scenario: Crear un issue avisa sin exigir aprobación

- **Given** la herramienta `github`
- **When** se consulta el permiso de `create_issue`
- **Then** el permiso es `Notify`

#### Scenario: La definición documenta los obligatorios por operación

- **Given** la definición de la herramienta `github`
- **When** se inspecciona el esquema de `create_issue`
- **Then** `operation` está en el enum y es obligatorio
- **And** el esquema documenta `title` como obligatorio y `body`, `repo` y `labels` como opcionales
