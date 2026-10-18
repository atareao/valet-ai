# journal-api Specification

## Purpose

Los endpoints de sesión de la bitácora de Valet sobre `/api/journal`: listar por rango, leer un día,
crear o reemplazar la entrada del día y borrarla, con el día como clave y errores explícitos para lo
mal formado y lo inexistente.

## ADDED Requirements

### Requirement: La API SHALL exponer el listado y la lectura de la bitácora bajo `/api/journal`

La API SHALL exponer `GET /api/journal` y `GET /api/journal/{date}` con sesión OIDC (el middleware ya
la impone sobre `/api/*`). `GET /api/journal` SHALL aceptar los parámetros opcionales `from` y `to`
(fechas ISO `YYYY-MM-DD`, rango inclusivo) y un `limit` opcional, SHALL devolver `200` con un array
de entradas ordenadas **de más reciente a más antigua** y SHALL acotar el resultado por `limit`.
`GET /api/journal/{date}` SHALL devolver `200` con la entrada de ese día o `404` si ese día no tiene
entrada. Cada entrada SHALL exponer `id`, `profile_id`, `date`, `mood` (uno de
`great`/`good`/`neutral`/`bad`/`terrible`, o `null`), `content`, `achievements`, `struggles`, `tags`
(listas, nunca `null`), `created_at` y `updated_at`.

**Contracts:**

```jsonc
// GET /api/journal?from=2026-10-01&to=2026-10-18&limit=50  → 200
[
  { "id": "…", "profile_id": "default", "date": "2026-10-18", "mood": "good",
    "content": "…", "achievements": ["…"], "struggles": [], "tags": ["…"],
    "created_at": "…", "updated_at": "…" }
]
```

#### Scenario: El listado devuelve el rango de más reciente a más antigua

- **Given** entradas en varias fechas
- **When** se hace `GET /api/journal?from=2026-10-01&to=2026-10-18`
- **Then** la respuesta es `200` con las entradas del rango
- **And** la más reciente aparece primero

#### Scenario: El listado respeta el tope

- **Given** más entradas que el `limit` pedido
- **When** se hace `GET /api/journal?limit=1`
- **Then** la respuesta es `200` con una sola entrada
- **And** es la más reciente

#### Scenario: Leer un día con entrada

- **Given** una entrada para `'2026-10-18'`
- **When** se hace `GET /api/journal/2026-10-18`
- **Then** la respuesta es `200` con esa entrada

#### Scenario: Leer un día sin entrada

- **Given** un día sin entrada en la bitácora
- **When** se hace `GET /api/journal/2026-10-19`
- **Then** la respuesta es `404`

#### Scenario: Una fecha mal formada se rechaza en la lectura

- **Given** una `date` que no encaja en `YYYY-MM-DD` (o un `from`/`to` mal formado)
- **When** se hace `GET /api/journal/{date}` (o `GET /api/journal`)
- **Then** la respuesta es `400` con el motivo

#### Scenario: Sin sesión no se lee la bitácora

- **Given** una petición a `GET /api/journal` sin sesión
- **When** se procesa
- **Then** la respuesta es `401`

### Requirement: La API SHALL exponer la escritura y el borrado de la bitácora bajo `/api/journal/{date}`

La API SHALL exponer `PUT /api/journal/{date}` y `DELETE /api/journal/{date}` con sesión OIDC.
`PUT` SHALL crear o **reemplazar** la entrada del día —un upsert por `(profile_id, date)`— a partir
de un cuerpo con `content` obligatorio y `mood`, `achievements`, `struggles` y `tags` opcionales, y
SHALL responder `200` con la entrada resultante. `DELETE` SHALL borrar la entrada del día y responder
`204`; si ese día no tiene entrada, SHALL responder `404`. Una `date` mal formada o un `mood` fuera
del conjunto SHALL responder `400` con el motivo y SHALL NOT escribir nada. Un `content` ausente o
vacío SHALL responder `400`. El perfil se resuelve con `ProfilesRepo::get_or_create`, como el resto
de la API de sesión.

**Contracts:**

```jsonc
// PUT /api/journal/2026-10-18
// body: { "content": "…", "mood": "good", "achievements": ["…"], "struggles": [], "tags": ["…"] }
// → 200 { "id": "…", "profile_id": "default", "date": "2026-10-18", "mood": "good", … }

// DELETE /api/journal/2026-10-18  → 204 (o 404 si ese día no existe)
```

#### Scenario: `PUT` crea la entrada del día

- **Given** un día sin entrada y un cuerpo con `content`
- **When** se hace `PUT /api/journal/2026-10-18`
- **Then** la respuesta es `200` con la entrada creada
- **And** `GET /api/journal/2026-10-18` la devuelve

#### Scenario: `PUT` reemplaza la del mismo día sin duplicar

- **Given** una entrada ya guardada para `'2026-10-18'`
- **When** se hace `PUT /api/journal/2026-10-18` con otro relato
- **Then** la respuesta es `200` con el relato nuevo
- **And** sigue habiendo una sola entrada para ese día

#### Scenario: `DELETE` borra la del día

- **Given** una entrada para `'2026-10-18'`
- **When** se hace `DELETE /api/journal/2026-10-18`
- **Then** la respuesta es `204`
- **And** `GET /api/journal/2026-10-18` pasa a responder `404`

#### Scenario: `DELETE` de lo que no existe

- **Given** un día sin entrada
- **When** se hace `DELETE /api/journal/2026-10-19`
- **Then** la respuesta es `404`

#### Scenario: Un ánimo fuera del conjunto se rechaza sin escribir

- **Given** un cuerpo con `mood: "genial"`
- **When** se hace `PUT /api/journal/2026-10-18`
- **Then** la respuesta es `400` con el motivo
- **And** la entrada de ese día no se modifica

#### Scenario: Una entrada sin ánimo se acepta

- **Given** un cuerpo con `content` y sin `mood`
- **When** se hace `PUT /api/journal/2026-10-18`
- **Then** la respuesta es `200` con `mood` a `null`

#### Scenario: Sin sesión no se escribe la bitácora

- **Given** una petición a `PUT /api/journal/2026-10-18` sin sesión
- **When** se procesa
- **Then** la respuesta es `401`
