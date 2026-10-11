# Tasks

## 0. Preparación y aprobación

- [x] 0.1 Presentar este change (proposal + design + specs + tasks) y **esperar aprobación explícita** antes de tocar `Cargo.toml` (aprobado por el usuario tras presentarlo en la conversación).

## 1. RED: reproducir la ruptura

- [x] 1.1 Línea base con `jsonwebtoken` 9.3.1: `cargo test --lib auth::` → **31 passed / 0 failed**.
- [x] 1.2 RED reproducido: `jsonwebtoken = "11"` **sin** feature + `cargo update -p jsonwebtoken` → resuelve 11.1.0, y `cargo test --lib auth::` → **17 passed / 14 failed**, los 14 en pánico con `Could not automatically determine the process-level CryptoProvider from jsonwebtoken crate features` en `jsonwebtoken-11.1.0/src/crypto/mod.rs:124`.
- [x] 1.3 `cargo check --all-targets` en ese estado roto: **compila limpio**. Ninguna ruptura de API —`Algorithm` `non_exhaustive`, los `ErrorKind` y `EncodingKey::from_rsa_pem` compilan sin cambios—, así que no hay hallazgo que documentar en `design.md`.

## 2. GREEN: declarar el proveedor

- [x] 2.1 `Cargo.toml`: `jsonwebtoken = { version = "11", features = ["aws_lc_rs"] }`, sin `default-features = false`, para conservar `use_pem` y con ella `EncodingKey::from_rsa_pem`.
- [x] 2.2 `cargo check --all-targets` limpio **sin tocar `src/auth.rs`** (`git diff --stat src/ tests/` vacío).
- [x] 2.3 `cargo test --lib auth::` → **31 passed / 0 failed**: los 14 tests que entraban en pánico pasan.
- [x] 2.4 `cargo test --no-fail-fast` → **1165 passed / 0 failed / 1 ignored**, idéntico a la referencia de `development`.
- [x] 2.5 `cargo tree -i aws-lc-rs` confirma una sola instancia (1.18.1), la misma que ya usa `rustls`. El `Cargo.lock` incorpora exactamente cuatro paquetes respecto a la 9.3.1 —`signature`, `zeroize_derive`, `rand_core 0.6.4` y `untrusted 0.7.1`—, ninguno de ellos un backend criptográfico de reemplazo, y no se elimina ninguno.

## 3. Regresión de los invariantes de autenticación

- [x] 3.1 El rechazo de `alg=none` (`test_validate_id_token_rejects_alg_none`) y de la confusión de algoritmos HS256 (`..._rejects_hs256_algorithm_confusion`) siguen en verde.
- [x] 3.2 Sesión: la expiración estricta (`test_validate_session_rejects_expired_token`) y una firma manipulada (`test_validate_session_rejects_tampered_signature`) se rechazan.
- [x] 3.3 ID token: expiración con margen (`..._rejects_expired_token`), `iss`/`aud`/`nonce` incorrectos, firma mala (`..._rejects_bad_signature`) y JWKS en otro origen se rechazan, todos en verde.
- [x] 3.4 `tests/api/auth.rs` pasa **13/13**, incluido el helper que firma el ID token con RSA en formato PEM.

## 4. Cierre

- [x] 4.1 No fue necesario: ni el compilador ni los tests exigieron tocar `src/auth.rs`, así que no hay hallazgo que añadir a `design.md`.
- [x] 4.2 `cargo clippy --all-targets -- -D warnings` limpio y `cargo fmt --check` OK.
- [x] 4.3 `openspec validate --all --strict` → 47 passed / 1 failed, el `infra` preexistente; sin fallos nuevos, y `openspec validate jsonwebtoken-11 --strict` válido.
- [ ] 4.4 PR `feature/jsonwebtoken-11` ← `development`, CI en verde y merge.
- [ ] 4.5 Cerrar el PR #147 como superado, con un comentario que apunte al PR nuevo.
- [ ] 4.6 Archivar el change con `openspec archive jsonwebtoken-11`.
