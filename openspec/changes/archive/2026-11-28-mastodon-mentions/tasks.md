# Tasks: mastodon-mentions

Issue: #389. Orden TDD estricto; no escribir código de aplicación hasta que el spec esté aprobado.

- [x] 1. RED — tests unitarios de conversión HTML→texto, mapeo de errores y fail-closed de operación en `src/services/mastodon.rs` y `src/tools/mastodon.rs`; ejecutar `cargo test` y comprobar que fallan.
- [x] 2. GREEN — implementación mínima del servicio y de la herramienta; `cargo test` y `cargo check`.
- [x] 3. RED — tests de dedupe de avisos y de visibilidad `direct` forzada; ejecutar y comprobar fallo.
- [x] 4. GREEN — migración `20261128000001_mastodon.sql`, tabla `mastodon_seen` y tool completa; registro en `src/lib.rs`.
- [x] 5. RED — tests de `status`, `check`, `token` y de omisión del token en `GET /api/settings` (`tests/api/mastodon.rs`).
- [x] 6. GREEN — handler, rutas y `SENSITIVE_KEYS`.
- [x] 7. RED — test de skill y catálogo 15 skills / 36 tools.
- [x] 8. GREEN — `Skill::Mastodon`, `SKILL_MASTODON_PROMPT` y guarda byte a byte en la migración.
- [x] 9. RED — tests Vitest de la sub-pestaña (token vacío, sonda, estado).
- [x] 10. GREEN — `api/mastodon.ts`, `useMastodon` y `MastodonSection.tsx`.
- [x] 11. REFACTOR — `cargo fmt --check`, `cargo clippy -- -D warnings`, `npx tsc --noEmit`, lint y verificación del presupuesto de bundle.
- [x] 12. Consolidar — marcar tareas, archivar el change con `openspec archive mastodon-mentions`, crear `docs/mastodon.md`, actualizar `AGENTS.md` y renombrar `plans/PLAN-032.md` a `plans/PLAN-032-ARCHIVED.md`.

Verificación final: `just check-all` en verde y `just check-spec` sin cambios activos tras el archive.
