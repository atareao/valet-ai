# Tasks

## 1. Baseline

- [x] 1.1 Registrar baseline por CLI: `cargo test --no-fail-fast` (passed/failed), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- [x] 1.2 Confirmar la causa: `execute` con `radius: 100000` o `2500.6` produce hoy `radius` sin acotar / `1000` respectivamente.

## 2. RED

- [x] 2.1 Test: `radius: 100000` → body con `locationBias.circle.radius == 50000`. Hoy envía 100000 → rojo.
- [x] 2.2 Test: `radius: 2500.6` → body con `radius == 2501`. Hoy `as_u64()` falla y aplica 1000 → rojo.
- [x] 2.3 Test: `radius: "cerca"` → body **sin** `locationBias`. Hoy aplica 1000 → rojo.
- [x] 2.4 Test: `radius: 0` → body **sin** `locationBias`. Hoy envía 0 → rojo.
- [x] 2.5 Verificar por CLI: 2.1–2.4 en rojo; el resto en verde.

## 3. GREEN

- [x] 3.1 `execute`: resolver `radius` con un helper (numérico > 0 → `round().clamp(1, 50000)`; si no, `None`).
- [x] 3.2 `search_text`: retirar el parámetro `included_type` y su rama `includedType`; actualizar las llamadas y tests.
- [x] 3.3 Tests GREEN: 2.1–2.4 pasan. Verificar por CLI: `cargo test --no-fail-fast` (0 failed) y `cargo check`.

## 4. REFACTOR y cierre

- [x] 4.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 4.2 Revisión con `rust-reviewer`.
- [ ] 4.3 PR a `development`; tras el merge, PR de archivado (`openspec archive fix-search-places-radius-validation`).

## 5. Verificación de integración

- [ ] 5.1 Tras desplegar: consulta real con `radius` grande (p. ej. 100000) no debe devolver 400.
