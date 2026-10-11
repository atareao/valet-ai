# Design

## Context

`jsonwebtoken` se usa en dos sitios y solo dos: `src/auth.rs` (cookie de sesión HS256, cookie de flujo HS256 y validación del ID token RS256 contra el JWKS) y `tests/api/auth.rs`.

Superficie exacta, verificada sobre el árbol — no hay `match` sobre `Algorithm`, ni uso de `insecure_disable_signature_validation`, `DecodingKey.as_bytes`, `EncodingKey.inner`, `Header.extras` ni `Jwk.thumbprint`, que son precisamente los cinco puntos que rompen en la 10/11:

- `Header::new(Algorithm::{HS256,RS256})`
- `EncodingKey::from_secret`, `EncodingKey::from_rsa_pem` (solo tests)
- `DecodingKey::from_secret`, `DecodingKey::from_jwk`
- `Validation::new`, `.leeway`, `.set_issuer`, `.set_audience`
- `jsonwebtoken::{encode, decode::<T>, decode_header}`
- `jsonwebtoken::errors::ErrorKind::{ExpiredSignature, InvalidIssuer, InvalidAudience}`
- `jsonwebtoken::jwk::JwkSet` y su `find`

Dos mediciones del punto de partida, que no deben confundirse:

- **La rama del PR #147, suite completa** (`cargo test`): **916 passed / 14 failed**, y los 14 son `auth::tests::*` con el pánico del `CryptoProvider`.
- **Este change, solo el módulo** (`cargo test --lib auth::`): línea base de **31 passed** con la 9.3.1, y **17 passed / 14 failed** en el RED sin la feature.

Restricción de construcción: la imagen se compila en `rust:1.98.1-alpine3.21` (musl) con `build-base`, `musl-dev` y `pkgconfig`. El proyecto **ya compila `aws-lc-rs` 1.18.1**: lo arrastra `rustls` 0.23, que entra por `reqwest` con `features = ["rustls"]`. El Dockerfile incluso lleva un `CFLAGS` con `-D` en lugar de `-include`, explicado justamente por los ficheros `.S` de `aws-lc-sys` en musl.

## Goals / Non-Goals

**Goals:**

- `jsonwebtoken` 11.1.0 en `Cargo.toml` y en `Cargo.lock`, con el `CryptoProvider` elegido explícitamente.
- Cero cambios de comportamiento en la autenticación: la suite completa en verde, incluidos los 14 tests que hoy fallan.
- No introducir un backend criptográfico nuevo ni duplicar `aws-lc-rs`: se reutiliza la instancia que ya usa `rustls`.
- Dejar el PR #147 cerrado y la dependencia al día.

**Non-Goals:**

- No se reclama cumplimiento FIPS ni se cambia el backend criptográfico del TLS.
- No se adoptan capacidades nuevas de la 10/11 (`dangerous::insecure_decode`, JWS, decodificación de byte slices, `KeyUtils`).
- No se toca `openspec/specs/oidc-auth/spec.md`: una spec describe comportamiento, y el comportamiento no cambia.
- No se cambia a `rust_crypto`.
- No se hace release en este change: va después, por su propio PR.

## Decisions

### 1. Backend criptográfico: `aws_lc_rs`, no `rust_crypto`

`aws_lc_rs` es la opción que no añade un backend criptográfico nuevo: `aws-lc-rs` 1.18.1 ya está resuelto en el `Cargo.lock` y ya pasa por musl en cada build de la imagen, así que jsonwebtoken **reutiliza la instancia que ya usa `rustls`** en lugar de introducir una segunda. El delta de dependencias tiene dos lecturas y conviene no mezclarlas. **Lo que añade la feature**, medido con un `diff` del lock entre el estado sin ella y el estado con ella, es un solo crate transitivo: `untrusted 0.7.1`, que se habilita dentro de `aws-lc-rs`. **Lo que añade el salto completo** respecto a la 9.3.1 son cuatro entradas —`signature`, `zeroize_derive`, `untrusted 0.7.1` y `rand_core 0.6.4`— de las que solo **tres se compilan**: `rand_core 0.6.4` es una dependencia **opcional** de `signature` que el lock anota y que `cargo tree -i rand_core@0.6.4` no encuentra en el árbol resuelto. `rust_crypto` traería siete crates nuevos (`ed25519-dalek`, `hmac`, `p256`, `p384`, `rand`, `rsa`, `sha2`), sería su estreno en musl dentro de este proyecto, obligaría a verificar que no choca con nuestro `rand = "0.10"` y dejaría tres stacks criptográficos conviviendo donde hoy hay dos.

*Alternativa descartada*: `rust_crypto`, por la regla de dedo «cripto puro en musl». El razonamiento es bueno en general y falso aquí: la evidencia del árbol lo desmiente.

### 2. Versión: 11.1.0

El coste de migración medido es el mismo en la 10 y en la 11, porque ningún cambio incompatible de ninguna de las dos toca nuestra superficie. Bajo ese empate gana la 11.1.0: incluye la zeroización parcial de claves de la 10.4.0, la corrección de la confusión de tipos `exp`/`nbf` de la 10.3.0, y evita otro salto de major dentro de unas semanas.

*Alternativas descartadas*: **10.3.0** (lo que propuso dependabot — salto mínimo y 22 M de descargas frente a 1,9 M, pero se queda sin las mejoras de la 10.4.0 y solo aplaza el salto de major) y **10.4.0** (la mejor de la familia 10, pero sigue dejando un major pendiente).

### 3. Rama propia `feature/*`, y el PR #147 se cierra

La convención del proyecto es `feature/*` ← `development` con merge commit. La rama de dependabot no la cumple, y sobre una rama de dependabot no se puede llevar el ciclo de TDD con su change. El #147 se cierra con un comentario que apunte al PR nuevo.

### 4. El RED de este change es la propia ruptura

No hay comportamiento nuevo que testear, así que el «test que falla» ya existe: subir `Cargo.toml` a la 11 **sin** la feature y comprobar que los 14 tests de `auth` entran en pánico. Ese es el contrato: los mismos 14 deben pasar a verde solo por efecto de declarar el proveedor. Cualquier otro cambio necesario en `src/auth.rs` que aparezca por el camino es señal de que el análisis de rupturas se dejó algo, y se trata como hallazgo, no como ajuste rutinario.

## Risks / Trade-offs

- **El análisis de rupturas puede estar incompleto.** Se hizo leyendo los cambios de la 10 y la 11 contra la superficie real, y ninguno de los puntos listados nos toca. Pero eso hay que **demostrarlo compilando**: `Algorithm` es `non_exhaustive` desde la 11 y hoy no hacemos `match` sobre él, que es exactamente el tipo de detalle que un `cargo check` desmiente en un segundo. Mitigación: la tarea 1.3 registra el resultado de compilar antes de tocar nada más.
- **Semántica de expiración: hay un cambio observable, y es a favor.** La 10.3.0 trae «Fix type confusion with exp/nbf when not required», que toca justo lo que validan nuestra sesión (`leeway = 0`) y nuestro ID token (`ID_TOKEN_LEEWAY_SECS`). Leyendo el `validation.rs` de la 10.3.0 y de la 11.1.0, el efecto concreto es que un token cuyo `exp` venía malformado y en la 9 podía atravesar la validación ahora se rechaza; ese rechazo cae en el comodín `_` de `src/auth.rs` y se traduce al mismo `AuthError` de siempre —`InvalidSession` en la sesión, `InvalidToken` en el ID token—, así que el **código HTTP no cambia** (401). Mitigación: los tests de expiración estricta y de expiración con margen son la red.
- **`err.kind()` está en el contrato.** El mapeo a `AuthError::{ExpiredToken, InvalidIssuer, InvalidAudience, InvalidSession}` depende de que esos `ErrorKind` sigan existiendo y significando lo mismo. Mitigación: los tests de expirados, `iss` y `aud`, más `cargo check` si alguna variante cambió de nombre.
- **`use_pem` sigue siendo feature por defecto** en la 10 y en la 11, y de ella depende `EncodingKey::from_rsa_pem` en los tests. Si upstream la cambiara, los tests romperían de forma ruidosa y no silenciosa: aceptable.
- **Elección criptográfica con consecuencias.** `aws_lc_rs` es el backend ya presente, y no se reclama FIPS ni se altera el del TLS. Si algún día se quisiera FIPS, la decisión se revisa entonces.
- **El delta de dependencias no es cero, y conviene no fingir que sí.** Medido: entran cuatro entradas respecto a la 9.3.1, de las que **tres se compilan** —`signature` y `zeroize_derive` con la 11, más `untrusted 0.7.1`, que la feature activa en `aws-lc-rs`—; la cuarta, `rand_core 0.6.4`, es opcional de `signature` y no llega a compilarse. `untrusted` es un crate pequeño y muy auditado, `aws-lc-rs` no se duplica, y ninguna es un backend criptográfico de reemplazo. La afirmación correcta de este change es «el backend criptográfico no añade dependencias», no «este change no añade dependencias».
- **Este change no toca la base de datos**, así que no hay migración de datos ni copia de seguridad en juego.
