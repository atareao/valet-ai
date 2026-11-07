# Tasks

## 1. Backend: `resource` derivado y diagnóstico visible (Rust, TDD)

- [x] 1.1 RED: añadir tests de resolución de `resource` (derivado `<base_url>/api`; base con `/` final ⇒ sin `//api`; override por ajuste y por entorno) y verificar que `cargo test` falla
- [x] 1.2 RED: añadir test de que el cuerpo del `client_credentials` incluye el par `resource` con la API resuelta, conservando `grant_type`, `client_id`, `client_secret` y `scope`, y verificar que `cargo test` falla
- [x] 1.3 RED: añadir test de `check_detail` con `TokenEndpoint`/`Upstream` que compruebe que el detalle incluye el mensaje del IdP y NO contiene `SECRET`, y verificar que `cargo test` falla
- [x] 1.4 RED: añadir test de `GET /api/oximeal/status` que compruebe el campo `resource`, y verificar que `cargo test` falla
- [x] 1.5 GREEN: implementar `oximeal_resource`/`OXIMEAL_RESOURCE` con default derivado en `OximealConfig`/`resolve_config`, añadir `resource` a `fetch_token`, a `StatusResponse` y el detalle redactado a `check_detail`, y verificar que `cargo test` pasa
- [x] 1.6 REFACTOR: limpiar y verificar `cargo test`, `cargo clippy -- -D warnings` y `cargo fmt --check` sin errores

## 2. Frontend: campo Resource y línea de estado (TS/React, TDD)

- [x] 2.1 RED: añadir tests del campo `oximeal_resource` (etiqueta «OxiMeal · Resource», placeholder, viaja en `PUT /api/settings`) y del `resource` en `OximealStatus` reflejado en la línea de estado, y verificar que `npx vitest run` falla
- [x] 2.2 GREEN: añadir el campo en `frontend/src/components/OximealSection.tsx`, el tipo `resource` en `OximealStatus` en `frontend/src/types/index.ts` y el reflejo del `resource` resuelto en la línea de estado del panel, y verificar que `npx vitest run` pasa
- [x] 2.3 REFACTOR: verificar `npx tsc --noEmit`, `npx vitest run`, `eslint` y `prettier` sin errores

## 3. Verificación

- [x] 3.0 Rellenar el `## Purpose` de `openspec/specs/oximeal-api/spec.md` y `openspec/specs/oximeal-service/spec.md` (hoy en `TBD`, causa de 2 fallos de `--strict`) con un resumen real de cada capacidad: `oximeal-api` = endpoints `/api/oximeal/*` (status, check, secret) sin exponer el secreto; `oximeal-service` = cliente de la Agent API de OxiMeal con token M2M y sus seis operaciones.
- [x] 3.1 Verificar que `openspec validate --all --strict` pasa sin errores ni warnings → 65 passed, 0 failed
- [x] 3.2 Ejecutar las auditorías `@rust-reviewer` y `@react-reviewer` y verificar que no dejan hallazgos bloqueantes → sin bloqueantes; mejoras menores registradas como follow-ups
- [ ] 3.3 Abrir PR a `development` y verificar la conexión real Valet↔OxiMeal: `GET /api/oximeal/check` devuelve `ok: true` con «conexión correcta»
