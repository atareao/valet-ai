# Tasks

## 0. Preparación y aprobación

- [x] 0.1 Presentar este change (proposal + specs + tasks) y **esperar aprobación explícita** antes de escribir código.
- [x] 0.2 Registrar la aplicación en `strava.com/settings/api` (Single Player Mode, un atleta); anotar `client_id`, `client_secret` y declarar la `redirect_uri` del callback (`/api/strava/callback`). No se versionan las credenciales.

## 1. RED: fijar el contrato

- [x] 1.1 Tests que fallan (OAuth): un `state` inválido o ausente en el callback **rechaza** el canje.
- [x] 1.2 Tests que fallan (OAuth): una respuesta con `error=access_denied` **no deja rastro** —ni tokens ni conexión— en `settings`.
- [x] 1.3 Tests que fallan (OAuth): un canje correcto guarda `strava_access_token`, `strava_refresh_token`, `strava_expires_at`, `strava_athlete_id` y `strava_scope` en `settings`.
- [x] 1.4 Tests que fallan (refresco con **rotación**): cada refresco persiste el refresh token **devuelto** por Strava, que sustituye al anterior.
- [x] 1.5 Tests que fallan (concurrencia): dos refrescos simultáneos del mismo atleta **no se pisan** (un único punto de refresco por atleta) y el refresh token final es el último devuelto.
- [x] 1.6 Tests que fallan (revocación): `POST /api/strava/disconnect` revoca en Strava y borra los tokens de `settings`; `GET /api/strava/status` no expone nunca los tokens.
- [x] 1.7 Tests que fallan (contrato de tools) con el cliente HTTP **mockeado**: `strava_recent_activities` (`before`, `after`, `page`, `per_page`, filtro de deporte Run/TrailRun/VirtualRun), `strava_activity_detail` (una actividad con vueltas y splits), `strava_activity_streams` (`keys`, `key_by_type`: ritmo/FC/cadencia/altitud) y `strava_athlete_stats` (perfil y totales de año/recientes). Todas de solo lectura y devolviendo un resumen útil.
- [x] 1.8 Tests que fallan (fallos): sin conexión → mensaje claro; `401` → un refresco y reintento y, si sigue, mensaje claro; `403`/`404` → mensaje claro; `429` → mensaje con el límite; sin errores crudos ni pánicos.
- [x] 1.9 Tests que fallan (tasa): una consulta repetida dentro de la ventana de caché **no sale a la red**.
- [x] 1.10 Tests que fallan (catálogo): el catálogo tiene **siete** skills y **diecisiete** herramientas; las cuatro `strava_*` están todas en `running`; el test de integridad pasa.

## 2. GREEN — Backend

- [x] 2.1 Servicio OAuth de Strava: `GET /api/strava/authorize` (redirect a `oauth/authorize` con `client_id`, `redirect_uri`, `response_type=code`, `state`, `scope`), `state` por intento y verificado; `GET /api/strava/callback` (canje contra `POST /oauth/token` con `grant_type=authorization_code`).
- [x] 2.2 `client_id`/`client_secret` en `settings` con respaldo `STRAVA_CLIENT_ID`/`STRAVA_CLIENT_SECRET`; claves de tokens en `settings` y migración que las siembra (idempotente).
- [x] 2.3 Refresco con rotación y **lock por atleta**: `grant_type=refresh_token`, persistir siempre el refresh token devuelto, reutilizar el access token si sigue vigente (> 1 h restante).
- [x] 2.4 `GET /api/strava/status` (conectado/atleta/scope, sin tokens) y `POST /api/strava/disconnect` (revoca en Strava y limpia).
- [x] 2.5 Las cuatro herramientas `strava_*` de solo lectura, con su cliente HTTP, resúmenes útiles y permisos de solo lectura (`NoConfirm`).
- [x] 2.6 Manejo de fallos accionable (sin conexión, `401` con refresco+reintento, `403`/`404`, `429`) sin errores crudos.
- [x] 2.7 Caché corta de las lecturas y respeto de `X-RateLimit-*` / `429` (sin polling).
- [x] 2.8 Skill `running` en el catálogo, registro de las cuatro herramientas y cableado en `src/lib.rs` y rutas.

## 3. REFACTOR / limpieza

- [x] 3.1 `cargo fmt` + `cargo clippy --all-targets -- -D warnings` (0 warnings).
- [x] 3.2 Revisar el catálogo: siete skills, diecisiete herramientas, sin nombres duplicados.
- [x] 3.3 Retirar andamiaje muerto si algún test o variante deja de usarse.

## 4. FRONTEND

- [x] 4.1 Nueva sección «Integraciones» en `SettingsDialog`: campos de `client_id`/`client_secret`, botón oficial «Connect with Strava», estado (conectada como \<atleta\> / no conectada) y desconectar.
- [x] 4.2 `tsc --noEmit` + lint frontend en verde; los tokens no se renderizan en ningún caso.

## 5. VERIFY / cierre

- [x] 5.1 `cargo test`, `npx vitest run` y `openspec validate strava-running --strict` en verde.
- [x] 5.2 Reviews `@rust-reviewer` y `@react-reviewer`; hallazgos aplicados.
- [x] 5.3 `openspec archive strava-running` y actualizar `AGENTS.md § V` si procede.
