# Change: Skill de running sobre la API de Strava (solo lectura)

## Why

Valet enruta hoy hacia seis dominios de dominio amplio (`agenda`, `pendientes`, `recuerdos`,
`entorno`, `web`, `widgets`) y no sabe nada del entrenamiento del atleta. La API de Strava expone la
actividad del corredor —salidas recientes, detalle de una actividad, series temporales y estadísticas
del atleta—, y esa información es exactamente lo que sostiene un dominio nuevo: **running**. La
integración es **de solo lectura**: se consulta la actividad del atleta, nunca se publica ni se
modifica nada en Strava. No se guardan actividades en Valet; lo único que persiste son las
credenciales OAuth y los tokens de acceso.

## What Changes

- **Nueva skill `running`** en el catálogo cerrado, que pasa de seis a **siete** skills, con cuatro
  herramientas: `strava_recent_activities`, `strava_activity_detail`, `strava_activity_streams` y
  `strava_athlete_stats`. El catálogo de herramientas pasa de trece a **diecisiete**.
- **OAuth2 dentro de la aplicación**, en nombre de un único atleta (Single Player Mode). `GET
  /api/strava/authorize` redirige a la autorización de Strava con un `state` por intento; `GET
  /api/strava/callback` canjea el `code` por tokens con `grant_type=authorization_code` y los guarda
  en `settings`.
- **Tokens en `settings` y refresco con rotación**: claves `strava_client_id` (respaldo
  `STRAVA_CLIENT_ID`), `strava_client_secret` (`STRAVA_CLIENT_SECRET`), `strava_refresh_token`,
  `strava_access_token`, `strava_expires_at`, `strava_athlete_id` y `strava_scope`. Strava **rota el
  refresh token** en cada refresco —el devuelto invalida al anterior—, así que el devuelto se
  persiste siempre. Un **único punto de refresco por atleta** (lock) evita que dos turnos
  concurrentes se pisen.
- **Consulta en vivo con caché corta**: las cuatro herramientas leen la API en vivo, sin polling y
  respetando `X-RateLimit-*` y el `429`; una caché corta evita repetir una lectura dentro de la
  misma ventana.
- **Fallos accionables**: sin conexión, `401`, `403`/`404`, `429` o error de red producen un mensaje
  claro para el usuario; nunca un error crudo ni un pánico.
- **Sección «Integraciones» en los ajustes**: botón oficial «Connect with Strava», campos de
  `client_id`/`client_secret`, estado de la conexión y desconexión. Los tokens **no** se muestran.
- **Sin widget**: no se añade ningún widget al catálogo.

## Impact

### Specs nuevas

- `tools/strava`: OAuth de solo lectura, persistencia y rotación de tokens en `settings`, estado y
  desconexión, contrato de las cuatro herramientas, manejo de fallos y respeto de los límites de
  tasa.
- `strava-ui`: sección de Integraciones en el diálogo de ajustes.

### Specs modificadas

- `orchestrator/skill-router`: el catálogo cerrado pasa de seis a siete skills (se añade `running`,
  que cubre las cuatro `strava_*`); el escenario de cobertura pasa de trece a diecisiete
  herramientas; la consulta del catálogo devuelve siete skills.
- `tools/registry`: el registry de producción incluye además las cuatro herramientas `strava_*`.

### Código

- `src/tools/` (las cuatro herramientas y el cliente HTTP de Strava), el servicio OAuth (state,
  canje, refresco con rotación y lock por atleta), las rutas de Strava (`/api/strava/*`), el catálogo
  de skills, `src/lib.rs`, `src/config.rs`, `src/db/repos/settings.rs` y una migración para sembrar
  las claves de `settings`.

### Frontend

- `SettingsDialog`: nueva sección «Integraciones».

### Datos

- **No se guardan actividades**: solo las credenciales y los tokens en la tabla `settings`.

### Requisito previo

- Registrar la aplicación en `strava.com/settings/api` (modo Single Player, un atleta) para obtener
  `client_id` y `client_secret`, y declarar la `redirect_uri` del callback.
