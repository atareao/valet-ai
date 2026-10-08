# Tasks

## 1. RED — test que falla

- [x] 1.1 Test nuevo en `tests/db/migrations.rs`: tras migrar, `ROUTER_ENABLED` vale `true` (hoy vale `false`).
- [x] 1.2 Confirmar que el test nuevo falla y que el resto del target `migrations` sigue verde.

## 2. GREEN — implementación

- [x] 2.1 Nueva migración `migrations/20261008000002_router_enabled_by_default.sql` que voltea `ROUTER_ENABLED` de `false` a `true` solo si sigue en el valor sembrado.
- [x] 2.2 Actualizar en `tests/db/migrations.rs` el valor sembrado esperado (`false` → `true`) con su comentario, incluido el del test de defaults del router.
- [x] 2.3 Hacer explícito el estado apagado en el test `router_disabled_exposes_all_tools` de `src/orchestrator/agent.rs`: fijar `ROUTER_ENABLED=false` en vez de asumir el default (un default no es un estado).

## 3. REFACTOR

- [x] 3.1 `cargo fmt` y `cargo clippy --all-targets -- -D warnings` sin warnings.

## 4. Verificación

- [x] 4.1 `cargo test --test migrations` en verde.
- [x] 4.2 `just check-all` en verde.
- [x] 4.3 Humo sobre una copia de la BD de desarrollo: aplicar la migración y comprobar que `ROUTER_ENABLED` pasa a `true`; y que sin la clave el enrutador queda apagado.

## 5. Cierre

- [x] 5.1 Review con `@rust-reviewer` (solo lectura) y hallazgos aplicados.
- [x] 5.2 PR a `development` y `openspec archive router-enabled-by-default`.
