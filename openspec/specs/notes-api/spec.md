# notes-api Specification

## Purpose
Los endpoints de sesión de las notas sobre `/api/notes` y `/api/notes/{id}`: listar (con filtro por
categoría y tope), crear, leer, actualizar de forma parcial y borrar, con el perfil resuelto de la
sesión, la categoría validada contra un conjunto cerrado y errores explícitos para lo mal formado y
lo inexistente.

## Requirements

### Requirement: La API SHALL exponer el listado y el alta de notas bajo `/api/notes`

La API SHALL exponer `GET /api/notes` y `POST /api/notes` con sesión OIDC (el middleware ya la impone
sobre `/api/*`); el perfil SHALL resolverse de la sesión, nunca del cuerpo. `GET /api/notes` SHALL
aceptar una `category` opcional (∈ `idea`/`journal`/`fact`) y un `limit` opcional, SHALL devolver
`200` con un array de notas **de más reciente a más antigua** (`created_at DESC`) y SHALL acotar el
resultado por `limit`. El `limit` SHALL valer `100` por defecto y `500` como máximo; un `limit` que no
sea un entero positivo o que pase de `500` SHALL responder `400` con el motivo; una `category` fuera
del conjunto SHALL responder `400`.

`POST /api/notes` SHALL crear una nota y responder `201` con ella. `content` SHALL ser obligatorio y
no vacío (recortado); su ausencia o vaciedad SHALL responder `400`. `category` SHALL ser opcional,
tomar `idea` por defecto y validarse contra `idea`/`journal`/`fact`; una categoría fuera del conjunto
SHALL responder `400`. `tags` SHALL ser opcional.

Cada nota SHALL exponer `id`, `profile_id`, `content`, `category`, `tags` (cadena o `null`),
`created_at` y `updated_at`.

**Contracts:**

```jsonc
// GET /api/notes?category=journal&limit=100  → 200
[
  { "id": "…", "profile_id": "default", "content": "…", "category": "journal",
    "tags": "viaje, comida", "created_at": "…", "updated_at": "…" }
]
```

#### Scenario: El listado devuelve las notas del perfil, de más reciente a más antigua

- **Given** notas de varias fechas para el perfil de la sesión, y una nota de otro perfil
- **When** se hace `GET /api/notes`
- **Then** la respuesta es `200` con las notas del perfil y **no** con la del otro perfil
- **And** la más reciente aparece primero

#### Scenario: El listado filtra por categoría

- **Given** notas de categorías distintas
- **When** se hace `GET /api/notes?category=journal`
- **Then** la respuesta es `200` con las notas de esa categoría

#### Scenario: Una categoría de filtro inválida se rechaza

- **Given** una `category` que no está en el conjunto
- **When** se hace `GET /api/notes?category=otra`
- **Then** la respuesta es `400` con el motivo

#### Scenario: El listado se acota por el limit y rechaza los límites inválidos

- **Given** más notas que el `limit` pedido
- **When** se hace `GET /api/notes?limit=1`
- **Then** la respuesta es `200` con una sola nota (la más reciente)
- **And** con `limit=0`, `limit=-1`, `limit=abc` o `limit=501` la respuesta es `400`

#### Scenario: El alta crea la nota

- **Given** un cuerpo con `content` no vacío
- **When** se hace `POST /api/notes`
- **Then** la respuesta es `201` con la nota creada y su `id`
- **And** la nota queda persistida con `profile_id` de la sesión y categoría `idea`

#### Scenario: El alta rechaza contenido vacío y categoría inválida

- **Given** un `content` vacío (o ausente), o una `category` fuera del conjunto
- **When** se hace `POST /api/notes`
- **Then** la respuesta es `400` con el motivo
- **And** no se crea ninguna nota

#### Scenario: Sin sesión no se listan ni se crean notas

- **Given** una petición a `/api/notes` sin sesión
- **When** se procesa
- **Then** la respuesta es `401`

### Requirement: La API SHALL exponer la lectura, la actualización y el borrado bajo `/api/notes/{id}`

La API SHALL exponer `GET /api/notes/{id}`, `PUT /api/notes/{id}` y `DELETE /api/notes/{id}` con
sesión OIDC. `GET` SHALL devolver `200` con la nota o `404` si no existe. `PUT` SHALL aceptar un
cuerpo **parcial** con `content`, `category` y/o `tags` —un campo **ausente** deja el valor anterior—
y SHALL devolver `200` con la nota actualizada; SHALL responder `400` si no viene ningún campo, si
viene un `content` vacío o si la `category` no está en el conjunto. `tags: ""` SHALL vaciar las
etiquetas. `DELETE` SHALL responder `204`.

Toda operación sobre `{id}` SHALL exigir que la nota pertenezca al perfil de la sesión: una nota
inexistente **o de otro perfil** SHALL responder `404` (nunca `403`), sin revelar su existencia.

#### Scenario: Leer una nota existente

- **Given** una nota del perfil
- **When** se hace `GET /api/notes/{id}`
- **Then** la respuesta es `200` con esa nota

#### Scenario: Leer una nota inexistente

- **Given** un `id` que no corresponde a ninguna nota
- **When** se hace `GET /api/notes/{id}`
- **Then** la respuesta es `404`

#### Scenario: La actualización es parcial

- **Given** una nota con `content`, `category` y `tags`
- **When** se hace `PUT /api/notes/{id}` con solo `{"content":"Corregido"}`
- **Then** la respuesta es `200` con el `content` nuevo
- **And** la `category` y los `tags` anteriores se conservan

#### Scenario: El `PUT` vacía las etiquetas con una cadena vacía

- **Given** una nota con etiquetas
- **When** se hace `PUT /api/notes/{id}` con `{"tags":""}`
- **Then** las etiquetas de la nota quedan vacías

#### Scenario: El `PUT` rechaza lo inválido

- **Given** un cuerpo sin ningún campo, o con `content` vacío, o con una `category` fuera del conjunto
- **When** se hace `PUT /api/notes/{id}`
- **Then** la respuesta es `400` con el motivo
- **And** la nota queda intacta

#### Scenario: El borrado elimina la nota

- **Given** una nota del perfil
- **When** se hace `DELETE /api/notes/{id}`
- **Then** la respuesta es `204`
- **And** la nota deja de aparecer en el listado

#### Scenario: Ninguna operación toca notas de otro perfil

- **Given** una nota cuyo `profile_id` no es el de la sesión
- **When** se hace `GET`, `PUT` o `DELETE /api/notes/{id}`
- **Then** la respuesta es `404`
- **And** la nota sigue existiendo sin cambios
