# dependencies Specification

## Purpose

Define las reglas de compatibilidad y seguridad que el código debe respetar al usar las
dependencias Rust del proyecto (axum, sqlx, reqwest, tower-http, thiserror, base64, etc.),
especialmente tras actualizaciones de versión mayor que introducen breaking changes.

Cubre dos invariantes críticos:

- **SQL dinámico**: con sqlx 0.9, cualquier query construida con un `String` dinámico debe
auditarse y envolverse explícitamente con `sqlx::AssertSqlSafe(...)`; los valores de usuario
van siempre por bind parameters.
- **Rutas axum**: con axum 0.8, los segmentos de captura usan la sintaxis `{param}`.

Además, fija el criterio de aceptación de una actualización de dependencias: el proyecto
de compilar sin errores, pasar el 100% de los tests y no emitir warnings de clippy.

## Requirements

### Requirement: SQL dinámico SHALL auditarse explícitamente con sqlx 0.9

**Given** el crate `sqlx` en versión 0.9  
**When** se construye una query con un `String` dinámico (no `&'static str`)  
**Then** SHALL envolverse con `sqlx::AssertSqlSafe(...)` antes de pasarla a `sqlx::query()` o `sqlx::query_scalar()`  
**And** el SQL dinámico SHALL construirse solo a partir de fragmentos literales controlados por el código, nunca de input de usuario sin sanitizar

#### Scenario: Query dinámica con bind parameters
**Given** un `String` `sql` construido con fragmentos literales y placeholders `?1`  
**When** se ejecuta `sqlx::query(sqlx::AssertSqlSafe(sql)).bind(profile_id)`  
**Then** la query SHALL compilar y ejecutarse sin error

#### Scenario: Query dinámica sin binds
**Given** un `String` `query_str` construido con fragmentos literales  
**When** se ejecuta `sqlx::query(sqlx::AssertSqlSafe(query_str))`  
**Then** la query SHALL compilar y ejecutarse sin error

### Requirement: Rutas axum SHALL usar la sintaxis de captura `{param}`

**Given** el crate `axum` en versión 0.8  
**When** se registra una ruta con un segmento de captura  
**Then** SHALL usarse la sintaxis `{param}` (p. ej. `/api/messages/{msg_id}`)  
**And** NO SHALL usarse la sintaxis obsoleta `:param`

#### Scenario: Router construido sin panic
**Given** el router de la aplicación con todas sus rutas registradas  
**When** se construye el router  
**Then** NO SHALL producirse panic por sintaxis de ruta inválida

#### Scenario: Captura de path param en runtime
**Given** una petición `GET /api/messages/{msg_id}` con un id válido  
**When** se enruta la petición  
**Then** el handler SHALL recibir el valor del path param

### Requirement: El proyecto SHALL compilar y pasar la suite de tests con las versiones actualizadas

**Given** las dependencias en sus versiones mayores actualizadas  
**When** se ejecuta `cargo check --all-targets`  
**Then** SHALL compilar sin errores  
**And** `cargo test` SHALL pasar el 100% de la suite, tanto la de librería como la de integración  
**And** `cargo clippy -- -D warnings` SHALL no emitir warnings

#### Scenario: Compilación limpia

- **Given** el árbol de código adaptado a las nuevas versiones
- **When** se ejecuta `cargo check --all-targets`
- **Then** SHALL finalizar con código de salida 0 y sin errores

#### Scenario: Suite de tests en verde

- **Given** el árbol de código adaptado a las nuevas versiones
- **When** se ejecuta `cargo test`
- **Then** SHALL pasar el 100% de los tests, librería e integración
- **And** NO SHALL haber tests fallidos

#### Scenario: Linter sin warnings

- **Given** el árbol de código adaptado a las nuevas versiones
- **When** se ejecuta `cargo clippy -- -D warnings`
- **Then** SHALL finalizar sin warnings

### Requirement: El proyecto SHALL registrar la extensión sqlite-vec sobre la instancia de libsqlite3-sys que usa sqlx

El proyecto SHALL declarar `sqlite-vec` con el feature `vec0` activado y `libsqlite3-sys = "0.37"` como **dependencia directa**, de modo que se registre la extensión sobre la misma instancia de `libsqlite3-sys` que usa `sqlx-sqlite` (Cargo unifica una única instancia). Al arrancar SHALL llamarse a `sqlite3_auto_extension(sqlite3_vec_init)` antes de abrir el pool, lo que cubre cada conexión al abrirse. `sqlite-vec 0.1.9` SHALL NOT declararse como dependiente de `libsqlite3-sys`.

**Given** el árbol de dependencias del proyecto  
**When** se resuelve `libsqlite3-sys`  
**Then** SHALL existir una **única** instancia compartida por el código del proyecto y `sqlx-sqlite`  
**And** `sqlite-vec` SHALL estar con el feature `vec0` activado  
**And** `libsqlite3-sys` SHALL ser dependencia directa

#### Scenario: Una única instancia de libsqlite3-sys
**Given** `libsqlite3-sys = "0.37"` como dependencia directa  
**When** se ejecuta `cargo tree -i libsqlite3-sys`  
**Then** aparece una única instancia `0.37.0`  
**And** es la misma que usa `sqlx-sqlite`

#### Scenario: La extensión queda registrada en cada conexión
**Given** la aplicación arrancada con `sqlite3_auto_extension(sqlite3_vec_init)`  
**When** se ejecuta `SELECT vec_version()` desde una conexión del pool y desde una conexión nueva  
**Then** ambas devuelven `v0.1.9`

### Requirement: El arranque SHALL fallar de forma explícita si la extensión sqlite-vec no carga

Si la extensión no está disponible, el sistema SHALL **no arrancar**, con un error explícito. Un asistente que arranca aparentemente bien pero sin memoria es un fallo silencioso inaceptable. La comprobación SHALL hacerse ejecutando `SELECT vec_version()` al arrancar.

**Given** el arranque de la aplicación  
**When** se ejecuta `SELECT vec_version()` como comprobación  
**Then** si la extensión responde, el arranque SHALL continuar  
**And** si la extensión no carga, el arranque SHALL fallar con un error explícito que indique qué falta (extensión no registrada / `vec0` no disponible)

#### Scenario: La extensión disponible permite arrancar
**Given** la extensión `sqlite-vec` registrada  
**When** se ejecuta `SELECT vec_version()` al arrancar  
**Then** devuelve la versión y el arranque continúa

#### Scenario: La extensión ausente aborta el arranque
**Given** una conexión sin la extensión `sqlite-vec` registrada  
**When** se ejecuta la comprobación de arranque  
**Then** el arranque falla  
**And** el error indica que la extensión no está registrada o que `vec0` no está disponible

### Requirement: El proveedor criptográfico de jsonwebtoken SHALL seleccionarse explícitamente

Con `jsonwebtoken` 11 la firma y la verificación delegan en un `CryptoProvider` que el crate SHALL NOT elegir por su cuenta: si no hay activada exactamente una de las features `aws_lc_rs` o `rust_crypto`, `encode` y `decode` compilan pero **entran en pánico en tiempo de ejecución**. El proyecto SHALL declarar la feature `aws_lc_rs` —el mismo backend que ya usa `rustls` a través de `reqwest`— y SHALL NOT declarar `rust_crypto`.

**Given** el crate `jsonwebtoken` en versión 11 declarado en `Cargo.toml`  
**When** se firma o se verifica un token  
**Then** SHALL existir exactamente un `CryptoProvider` seleccionado, sin depender del valor por defecto del crate

#### Scenario: Sin proveedor declarado la firma falla

- **Given** `jsonwebtoken` 11 declarado sin la feature `aws_lc_rs` ni `rust_crypto`
- **When** se ejecuta `encode` o `decode`
- **Then** la llamada entra en pánico en lugar de devolver un resultado

#### Scenario: Con aws_lc_rs se firma y se verifica

- **Given** `jsonwebtoken` 11 declarado con la feature `aws_lc_rs`
- **When** se emite una cookie de sesión HS256 y se valida a continuación
- **Then** la firma y la verificación SHALL completarse sin pánico

#### Scenario: Un único backend criptográfico en el árbol

- **Given** el árbol de dependencias con `jsonwebtoken` 11 y la feature `aws_lc_rs`
- **When** se resuelve `aws-lc-rs`
- **Then** SHALL existir una **única** instancia, la 1.18.1, la misma que ya usa `rustls`
- **And** activar la feature SHALL NOT introducir `rust_crypto` ni ninguna de sus dependencias
