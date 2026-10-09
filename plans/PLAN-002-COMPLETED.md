# PLAN PENDIENTE — OIDC (PocketID), despliegue en VPS y cierres recientes

**Proyecto:** Valet (Rust/Axum + React + SQLite)
**Fecha:** 2026-10-06
**Estado:** 🟢 OIDC verificado en el VPS y mergeado; producción tras Traefik mergeada. Pendiente solo la decisión del autoarranque del host (Bloque D).

## Cerrado — 2026-10-06: OIDC (PocketID) + producción tras Traefik

- Change `oidc-auth`: verificación end-to-end **6.4 ✓** en el VPS con PocketID real (`login → /api/auth/me → rutas protegidas → logout SSO`; tras el logout el proveedor **vuelve a pedir credenciales**). Tarea **7.2** (archivado) ✓.
  `openspec validate --archived` → `2026-10-05-oidc-auth` ✓ (53/53).
- Mergeado a `development`: **PR #131** (OIDC) → merge commit `0603895`.
- Change `prod-traefik` (producción tras Traefik, PocketID **externo**, **un único `Dockerfile`**): archivado y mergeado → **PR #133**, merge `dd9ae71`.
- Cierre del change `prod-traefik` en **PR #134** (marca 6.4/7.2 del OIDC archivado), merge `fe66401`.
- Ramas `feature/oidc-auth`, `feature/prod-traefik` y `docs/close-oidc-verification` borradas (local y remoto).
- Change `persist-rendered-widgets` **cerrado**: cierre en **PR #135** (marca 4.2/4.3/4.4), merge `ab33579`. Implementación ya en `development` (PR #130, `c7de43f`) y archivado (`20fe2fa`); verificado que el widget renderizado **reaparece al recargar**.
- `openspec validate --archived` → `persist-rendered-widgets` ✓ sin tareas pendientes.

## Cerrado — 2026-10-06: esquemas de tools (`improve-tool-schemas`)

- Change implementado con SDD + TDD y **archivado** en `openspec/changes/archive/2026-10-05-improve-tool-schemas/`.
- Mergeado a `development`: **PR #132** (merge commit `85e2c23`).
- Alcance: descripciones en español (`web_search`, `search_places`, `notes`, `tasks`, `reminders`, `geo`, `current_*`), obligatorios por operación, `search_places` con `latitude`/`longitude` opcionales, `notes` sin categoría `todo` (BD intacta), `render_widget.data` con `anyOf` (sigue opcional), `weather` guía a `geocode`, `unified_search` en lenguaje natural.
- Verificado: `just check-all` verde, `clippy` 0 warnings, `openspec validate --specs` 39/39, revisión `@rust-reviewer` sin críticos y **validación en uso real** (los tools funcionan correctamente).

---

## Estado actual (verificado 2026-10-06)

- `openspec list` → **sin changes activos**.
- `openspec validate --specs` → **39/39**; `openspec validate --archived` → los changes recientes (`oidc-auth`, `persist-rendered-widgets`, `prod-traefik`) ✓ sin tareas pendientes.
- `development` (HEAD `ab33579`) contiene: OIDC con logout SSO, y `docker-compose.prod.yml` como **un único servicio `valet`** (el `Dockerfile` monolítico, SPA+API en `:3000`) detrás de Traefik con TLS, **sin puertos publicados**; PocketID **externo** vía `AUTH_ISSUER_URL`. Plantilla `.env.prod.sample` incluida.
- PRs #131, #133 y #134 **MERGED** a `development`.

---

## Bloque A — Verificación end-to-end en el VPS (tarea 6.4) [CERRADO]

- [x] Login redirige correctamente a PocketID y vuelve al callback.
- [x] Sesión válida en cookie HttpOnly; `/api/auth/me` responde con identidad.
- [x] Rutas `/api/*` protegidas responden 401 sin sesión.
- [x] Logout limpia la sesión y cierra el SSO (PocketID vuelve a pedir credenciales).
- [x] `AUTH_ENABLED=true` sin las variables requeridas **aborta el arranque** (fail-closed).

## Bloque B — Revisión y merge del PR #131 [CERRADO]

- [x] PR #131 revisado (diff, CI verde).
- [x] CI de `development` verde.
- [x] Mergeado a `development` (`0603895`).

## Bloque C — Contingencia si falla la tarea 6.4

No aplica: la verificación pasó. Se mantiene como referencia: si algo fallara, **no reabrir** el change archivado; abrir un **delta nuevo** con SDD + TDD.

## Bloque D — Infra aplazada: autoarranque tras reinicio del host (NO ejecutar sin nueva orden)

**Problema:** `docker-compose.yml` no declara `restart:`; el contenedor quedó con `RestartPolicy: no`; `podman-restart.service` está `disabled`. Ya ocurrió una caída de ~9 h 30 min tras un reinicio del host.

**Arreglo viable, ya comprobado** (requiere orden explícita del usuario):
1. Añadir `restart: unless-stopped` al servicio en `docker-compose.yml`.
2. `systemctl --user enable --now podman-restart.service`.
   - La unidad filtra por `should-start-on-boot=true`, que cubre `always` y `unless-stopped`.

**Restricción:** `docker-compose.prod.yml` **no se toca** sin orden explícita.

## Bloque E — Pulido menor (opcional, aplazado)

- [ ] Valorar los mensajes de error del flujo de auth (claridad para el operador).

---

## Definición de Hecho (DoD)

- [x] Tarea 6.4 verificada en el VPS y documentada.
- [x] PR #131 mergeado a `development`.
- [x] Change `prod-traefik` archivado y mergeado.
- [ ] Decisión tomada (sí/no) sobre el autoarranque del host (Bloque D).
- [x] `openspec list` sin changes activos.

---

## Notas de continuidad

- El change `oidc-auth` está archivado: cualquier arreglo posterior es un **delta nuevo**, no una reapertura.
- La imagen de producción es **un único servicio** (`valet`, `Dockerfile` monolítico); no hay nginx intermedio ni puertos publicados: el único ingress es el Traefik existente por la red externa.
- `PLAN-PENDIENTE.md` sigue **sin trackear** en git.
