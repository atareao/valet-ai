# Design: github-integration

## Decisiones

### D1 — Token Personal Access Token **clásico**, cabecera `Authorization: Bearer`

El issue recomendaba un token *fine-grained* (limitado por repositorio). El usuario eligió un
**Personal Access Token clásico**. Se implementa con la cabecera `Authorization: Bearer <token>`,
que GitHub admite para ambos tipos, de modo que no hay que tocar la cabecera si algún día se cambia
de tipo. Además, el clásico simplifica la configuración: sirve para cualquier repositorio del
usuario sin reemitirlo por repo.

### D2 — Una sola operación: `create_issue`

La herramienta es multioperación (molde `oximeal`/`zennotes`/`calendar`, con parámetro `operation`),
pero hoy declara **solo** `create_issue`. Listar o buscar issues queda **fuera de alcance**: añade
scope de lectura y superficie de UI sin resolver una necesidad concreta. Si en el futuro se añade
una operación destructiva (cerrar, editar), deberá declarar `ExplicitApproval`; `create_issue` no
es destructiva.

### D3 — Repo destino: ajuste `github_default_repo` + override por parámetro `repo`

El repositorio se resuelve en este orden: el parámetro `repo` de la llamada y, si falta, el ajuste
`github_default_repo`. Si no hay ni uno ni otro, la herramienta falla con un mensaje accionable
**sin salir a la red**: un error de validación no es un fallo de GitHub.

Alternativas descartadas: obligar siempre al modelo a pasar `repo` (incómodo y propenso a errores)
o deducirlo del contexto (no fiable). El ajuste cubre el caso común —un repo de trabajo— y el
parámetro permite apuntar a otro cuando haga falta.

### D4 — Labels: ajuste `github_default_labels` + override por parámetro `labels`

`github_default_labels` es una lista separada por comas que se aplica por defecto. El parámetro
`labels` de la llamada **sustituye** al ajuste (no lo suma): el modelo maneja el conjunto completo,
igual que `set_label` de OpenTodo compone el array entero. Si ni el ajuste ni el parámetro tienen
labels, se crea el issue **sin** labels.

### D5 — Permiso `Notify` para `create_issue`

Crear un issue es una escritura, pero **no** es destructiva ni irreversible desde la perspectiva de
Valet (un issue se puede cerrar o borrar en GitHub). `Notify` avisa sin exigir aprobación explícita,
igual que `add_shopping_items` y `create_recipe` de OxiMeal.

### D6 — La sonda consulta `GET https://api.github.com/user`

`GET /api/github/check` obtiene el usuario autenticado —no lee los ajustes—, que es la prueba mínima
de que el token es válido y de que GitHub responde. Es el molde de las sondas de ZenNotes, OpenTodo,
OxiMeal y el blog: **siempre `200`** con `{ok, detail}` y **nunca** el token. La URL del API de
GitHub (`https://api.github.com`) es **fija**, no un ajuste: no hay instancia self-hosted de GitHub
que configurar.

### D7 — Validación de `repo` y `labels` sin red

La herramienta valida antes de tocar la red: `title` no vacío; `repo` con forma `owner/name`
(exactamente una barra, ambos tramos no vacíos); labels recortados y sin vacíos. Un argumento
inválido produce `InvalidArguments` con un mensaje **accionable**, sin petición HTTP.

### D8 — El token solo vive en `settings`, por una puerta dedicada

`github_token` es sensible (`SENSITIVE_KEYS`): `GET /api/settings` lo omite y `PUT /api/settings` lo
ignora; la **única** puerta de escritura es `PUT /api/github/token`. No hay respaldo por variable de
entorno (la configuración de `settings` es la única fuente) ni se siembra en la migración, de modo
que el token nunca acaba en el repositorio ni en el historial de migraciones. Es el molde de
`zennotes_token`, `opentodo_token`, `oximeal_client_secret` y `blog_app_password`.

### D9 — Sin reintentos; solo `201` es éxito

El servicio hace **una** petición y no reintenta. Un `201 Created` es el único resultado de éxito;
cualquier otro estado se traduce a un error tipado. Reintentar una escritura sin idempotencia
arriesgaría issues duplicados.

## Traducción de errores

| Estado de GitHub | Variante | Mensaje (accionable, sin token) |
|:---|:---|:---|
| sin token | `NotConfigured` | indica dónde configurarlo |
| args inválidos | `InvalidArguments` | explica el campo concreto |
| `401`/`403` | `Unauthorized` | el token no es válido o no tiene permiso |
| `404` | `NotFound` | el repositorio no existe o el token no lo ve |
| `422` | `InvalidInput` | GitHub rechazó los datos (p. ej. label inexistente) |
| `429` | `RateLimited` | se ha superado el límite de peticiones |
| `5xx`/fallo de red | `Upstream` | GitHub no responde |

## Alternativas descartadas

- **Token fine-grained**: descartado por decisión del usuario (D1).
- **`list_issues`/`search_issues`**: fuera de alcance (D2).
- **Repo obligatorio por parámetro**: descartado (D3).
- **Respaldo por variables de entorno `GITHUB_*`**: descartado; `settings` es la única fuente (D8).
- **Reintentos con backoff**: descartado; sin idempotencia duplicaría issues (D9).
