## ADDED Requirements

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

## MODIFIED Requirements

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
