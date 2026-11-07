# PLAN-022 — Parámetro `resource` (RFC 8707) en el token M2M de Valet hacia OxiMeal

- **Estado:** 🔄 En curso — código, tests y auditorías hechos; pendiente PR, release y verificación real.
- **Fecha:** 2026-11-07.
- **Change de OpenSpec:** `oximeal-m2m-resource` (sin archivar todavía).
- **Rama:** `feature/oximeal-m2m-resource`.

---

## 1. Objetivo

Que Valet **conecte de verdad** con OxiMeal. La petición `client_credentials`
que hace Valet contra Pocket ID no enviaba el parámetro `resource` (RFC 8707),
así que Pocket ID rechazaba los scopes `agent:*` con `invalid_scope` y Valet lo
mostraba como «el IdP no entregó token». Objetivo: enviar `resource=<API>`
(resuelto con precedencia ajuste → entorno → derivado `<base_url>/api`), exponer
el `resource` en la sonda de estado y **mostrar el detalle redactado** de los
fallos del IdP y de OxiMeal para que el diagnóstico deje de ser opaco.

---

## 2. Checklist

### Spec (SDD)

- [x] Change OpenSpec `oximeal-m2m-resource` creado con deltas para
      `oximeal-service`, `oximeal-api` y `frontend`, y validado con
      `openspec validate oximeal-m2m-resource --strict`.

### Backend (Rust, TDD)

- [x] RED: tests de resolución de `resource` (derivado `<base_url>/api`; base con
      `/` final ⇒ sin `//api`; override por ajuste y por entorno), de que el
      cuerpo de `client_credentials` lleva el par `resource`, de `check_detail`
      con detalle redactado y de `resource` en `GET /api/oximeal/status`.
- [x] GREEN: `OximealConfig.resource`, `resolve_resource`, `resource` en
      `fetch_token`, `resource` en `StatusResponse` y `check_detail → String`
      con el detalle redactado.
- [x] REFACTOR: `cargo test`, `cargo clippy -- -D warnings` y
      `cargo fmt --check` limpios.
- [x] Backend: `cargo test --workspace` con **0 fallos** y 12 tests nuevos.

### Frontend (TS/React, TDD)

- [x] RED: tests del campo `oximeal_resource` (etiqueta «OxiMeal · Resource»,
      placeholder, viaja en `PUT /api/settings`) y del `resource` en
      `OximealStatus` reflejado en la línea de estado.
- [x] GREEN: campo en `OximealSection.tsx`, tipo `resource` en `types/index.ts`
      y reflejo del `resource` resuelto en la línea de estado.
- [x] REFACTOR: `tsc`, `eslint` y `vitest` limpios.
- [x] Frontend: `vitest` **696 tests / 79 ficheros** verdes.

### Verificación

- [x] Auditorías `@rust-reviewer` y `@react-reviewer` **sin bloqueantes**;
      mejoras menores registradas como follow-ups (en
      `docs/oximeal-m2m-resource.md`).
- [x] `openspec validate --all --strict` → **65 passed, 0 failed**.
- [x] Tareas 3.0 (Purpose de las specs), 3.1 (validación strict) y 3.2
      (auditorías) del change marcadas.
- [ ] Tarea 3.3: abrir PR a `development`.
- [ ] Release y despliegue de la imagen nueva en `co1`.
- [ ] Verificación real del `check`: `GET /api/oximeal/check` →
      `{ok:true, detail:"conexión correcta"}` contra un OxiMeal accesible.
- [ ] Archivar el change (`openspec archive oximeal-m2m-resource`) y renombrar
      este plan a `PLAN-022-ARCHIVED.md`.

---

## 3. Ficheros

**Código:** `src/services/oximeal.rs`, `src/handlers/oximeal.rs`,
`src/tools/oximeal.rs` (helper de test), `frontend/src/types/index.ts`,
`frontend/src/components/OximealSection.tsx`,
`frontend/src/components/SettingsDialog.tsx`.

**Documentación:** `docs/oximeal-m2m-resource.md` (esta feature),
`openspec/changes/oximeal-m2m-resource/`.

---

## 4. Follow-ups

Registrados en `docs/oximeal-m2m-resource.md` §Follow-ups: (1) `resolve_config`
lee `base_url` dos veces; (2) normalización asimétrica del `resource` explícito;
(3) el brazo `_` de `check_detail` sin detalle; (4) `redact` no sustituye el
token M2M; (5) sin test del reset de `oximeal_resource` en el formulario.
