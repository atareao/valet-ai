# Proposal

## Why

El PR #147 de dependabot (`jsonwebtoken` 9.3.1 → 10.3.0) lleva en rojo desde el 9 de octubre: **14 tests de `auth` entran en pánico en tiempo de ejecución**. La causa no es un cambio de firma de la API sino el `CryptoProvider`: a partir de la 10, `jsonwebtoken` no elige backend criptográfico por su cuenta y, si no hay activada exactamente una de las features `aws_lc_rs` o `rust_crypto`, `encode` y `decode` fallan con `Could not automatically determine the process-level CryptoProvider from jsonwebtoken crate features`. El crate **compila**, así que el fallo no aparece hasta que se ejecutan los tests — de ahí que el PR parezca correcto y no lo sea.

Quedarse en la 9.3.1 no sale gratis: la 10.4.0 zeroiza parcialmente `EncodingKey` y `DecodingKey` al soltarse, y la 10.3.0 corrige una confusión de tipos entre `exp` y `nbf`. Y como el coste de migrar resulta ser el mismo en cualquier versión de la familia 10/11, se salta directamente a la última estable (11.1.0) en lugar de quedarse a un salto de otro major.

## What Changes

- `Cargo.toml`: `jsonwebtoken = "9"` → `jsonwebtoken = { version = "11", features = ["aws_lc_rs"] }`. La feature `use_pem` sigue viniendo por defecto (`default = ["use_pem"]` tanto en la 10 como en la 11), que es lo que sostiene `EncodingKey::from_rsa_pem` en los tests.
- `Cargo.lock`: resolución de `jsonwebtoken` a 11.1.0. El delta medido son **cuatro entradas nuevas**, de las que **tres se compilan**: `signature` y `zeroize_derive` (proc-macro de `zeroize`, ya presente) entran con la 11, y `untrusted 0.7.1` lo activa la feature `aws_lc_rs` dentro de `aws-lc-rs`. La cuarta, `rand_core 0.6.4`, es una dependencia **opcional** de `signature` que el lock anota pero que **no se compila** (`cargo tree -i rand_core@0.6.4` no la encuentra). Ninguna es un backend criptográfico nuevo ni un sustituto: `aws-lc-rs` sigue siendo la **única** instancia, la 1.18.1, la misma que ya usa `rustls` 0.23 a través de `reqwest`. Para comparar, `rust_crypto` habría traído siete crates más. No se elimina ningún paquete: `ring` se queda, sostenido por `quinn-proto` y `rustls-webpki`.
- **NO cambia ningún comportamiento observable.** La superficie usada es idéntica en la 11 y ninguno de los cambios incompatibles de la 10/11 la toca: `Header::new`, `EncodingKey::from_secret`/`from_rsa_pem`, `DecodingKey::from_secret`/`from_jwk`, `Validation::new`/`.leeway`/`.set_issuer`/`.set_audience`, `encode`, `decode::<T>`, `decode_header`, `ErrorKind::{ExpiredSignature, InvalidIssuer, InvalidAudience}` y `jwk::JwkSet`.
- El PR #147 SHALL cerrarse como **superado**: el proyecto trabaja por `feature/*` desde `development`, y la rama de dependabot no cumple esa convención.
- Corrección adyacente en la spec de `dependencies`: su criterio de aceptación arrastra un recuento de tests obsoleto («510 lib + integración») cuando hoy son 1165.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `dependencies`: se añade el requisito de seleccionar explícitamente el `CryptoProvider` al usar `jsonwebtoken` 11, y se corrige el recuento de tests obsoleto del requisito de aceptación de actualizaciones de dependencias.

## Impact

- **Ficheros**: `Cargo.toml` y `Cargo.lock`.
- **Código afectado**: `src/auth.rs` y `tests/api/auth.rs` son los únicos que usan el crate. No se prevé tocarlos; si el compilador exige algún ajuste, será por una ruptura que el análisis no detectó y se tratará como hallazgo.
- **Invariantes en juego** (ya implementados y testeados, y SHALL seguir en verde): rechazo de `alg=none`, rechazo de la confusión de algoritmos HS256, expiración estricta en la sesión (`leeway = 0`), expiración con margen en el ID token, `iss`/`aud`/`nonce` incorrectos, firma manipulada y JWKS en otro origen.
- **Build**: el backend criptográfico elegido (`aws_lc_rs`) ya se compila hoy en Alpine/musl, así que el tiempo de construcción no cambia.
- **PR**: cierra el #147.
