# Tasks

## 0. Preparación y aprobación

- [ ] 0.1 Presentar este change (proposal + design + specs + tasks) y **esperar aprobación explícita** antes de escribir código.

## 1. RED: fijar el contrato

- [ ] 1.1 Tests que fallan (`src/services/strava.rs`, `wiremock`): un `403` con `Application/Status/Inactive` se traduce a `StravaError::ApplicationInactive` con el mensaje accionable (suscripción + `https://www.strava.com/settings/api`).
- [ ] 1.2 Tests que fallan: un `403` con otro `message`/`errors[]` se traduce a `StravaError::Http` incluyendo el `message` y el `field`/`code`, sin perder el código HTTP.
- [ ] 1.3 Tests que fallan: un cuerpo vacío o no JSON conserva el código HTTP y no provoca pánico.
- [ ] 1.4 Tests que fallan: el mismo parseo se aplica a un fallo del canje (`authorization_code`) y del refresco (`refresh_token`) contra `/oauth/token`.
- [ ] 1.5 Tests que fallan: `Strava::check` devuelve el atleta cuando `/athlete` responde `200`, y `ApplicationInactive` cuando responde `403` inactiva.
- [ ] 1.6 Tests que fallan: `Strava::check` **no se sirve de la caché** (dos comprobaciones seguidas = dos peticiones de red).
- [ ] 1.7 Tests que fallan: `disconnect` borra los tokens **siempre**; con revocación `2xx` no hay aviso, y con revocación fallida devuelve el aviso del motivo.
- [ ] 1.8 Tests que fallan (handler, `tests/api/`): `GET /api/strava/check` responde `200` con `{ok:true,...}` y con `{ok:false,error:"..."}`; sin tokens en el cuerpo.
- [ ] 1.9 Tests que fallan (handler): `POST /api/strava/disconnect` responde `{connected:false}` y, si la revocación no se confirma, `{connected:false, warning:"..."}`.
- [ ] 1.10 Tests que fallan (frontend, `vitest` + testing-library): «Probar conexión» invoca el endpoint y pinta la confirmación con el atleta o el mensaje del servidor; el `scope` concedido se muestra y avisa si falta `activity:read_all`; el aviso de desconexión no confirmada se muestra.

## 2. GREEN — Backend

- [ ] 2.1 `StravaError::ApplicationInactive` y el parseo tolerante del cuerpo de error de Strava (`message` + `errors[]` con `resource`/`field`/`code`).
- [ ] 2.2 `api_get` incorpora el cuerpo al error; `401` con refresco+reintento y `429` conservan su comportamiento.
- [ ] 2.3 El parseo del cuerpo se reutiliza en los fallos del canje y del refresco.
- [ ] 2.4 `Strava::check` (consulta `/athlete` sin caché) y la ruta `GET /api/strava/check` con respuesta `200` siempre.
- [ ] 2.5 `disconnect` comprueba la revocación y devuelve `{connected:false, warning?}`; los tokens se borran siempre.

## 3. GREEN — Frontend

- [ ] 3.1 `types/index.ts` y `api/client.ts`: `StravaCheckResult`, `checkStrava()` y `warning` en la desconexión.
- [ ] 3.2 `useStrava`: `check()`, `checking` y el resultado; `disconnect()` expone el aviso.
- [ ] 3.3 `StravaIntegration`: botón «Probar conexión» con su resultado, `scope` visible con aviso si falta `activity:read_all`, y aviso de desconexión no confirmada.

## 4. REFACTOR / limpieza

- [ ] 4.1 `cargo fmt` + `cargo clippy --all-targets -- -D warnings` (0 warnings).
- [ ] 4.2 `npx tsc --noEmit` + lint de frontend en verde.
- [ ] 4.3 Sin código muerto, sin variantes sin usar y sin claves nuevas en `settings`.

## 5. VERIFY / cierre

- [ ] 5.1 `cargo test`, `npx vitest run`, `tsc --noEmit`, lint y `openspec validate strava-diagnostics --strict` en verde.
- [ ] 5.2 Reviews `@rust-reviewer` y `@react-reviewer`; hallazgos aplicados.
- [ ] 5.3 `openspec archive strava-diagnostics` y actualizar `plans/PLAN-003.md` (Tema 4) y `AGENTS.md § V` si procede.
