# Change: Diagnóstico de la conexión con Strava (errores accionables y comprobación real)

## Why

El 2026-10-10 la integración de Strava de una instancia en producción devolvía **403 en todas las
llamadas**, con la skill `running` enrutando correctamente y los tokens guardados. El diagnóstico
requirió sacar el `access_token` de la base de datos con `sqlite3` y preguntarle a la API con `curl`,
porque **Valet descartaba el cuerpo de la respuesta de Strava**: el error que veía el usuario era
`error de Strava: Strava respondió 403 al consultar /athlete`, sin ninguna pista sobre la causa.

La causa real viajaba en el cuerpo: `{"message":"Forbidden","errors":[{"resource":"Application","field":"Status","code":"Inactive"}]}`
— la aplicación de Strava estaba **inactiva** porque la cuenta propietaria no tenía una suscripción
activa (requisito de Strava desde el 1 de julio de 2026). Es un estado que el usuario puede resolver por
sí mismo en `strava.com/settings/api`, pero **solo si Valet se lo dice**.

Además, la interfaz de ajustes mostraba «Conectada como \<atleta\>» mientras *todas* las consultas
fallaban: el estado se lee de `settings`, sin comprobar nunca contra Strava. La desconexión, por
último, ignoraba el resultado de la revocación (`let _ =`), de modo que un fallo al revocar dejaba la
conexión viva en Strava sin que nadie se enterase.

## What Changes

- **Errores accionables de verdad**: ante un fallo HTTP de Strava (distinto de `401` y `429`), Valet
  **lee el cuerpo** de la respuesta y lo incorpora al mensaje: el `message` y los `errors[]` con
  `field`/`code`. Ante `{"resource":"Application","field":"Status","code":"Inactive"}` se devuelve un
  error **específico y accionable** que explica que la aplicación de Strava está inactiva y que hay que
  reactivarla en `https://www.strava.com/settings/api` (la cuenta propietaria necesita suscripción).
- **Comprobación real de la conexión**: nuevo `GET /api/strava/check` que consulta `/athlete` contra
  Strava **en el momento, sin caché**, y responde siempre `200` con `{ok, athlete_id, athlete_name,
  error}`: `ok: true` con el atleta, o `ok: false` con el mensaje accionable. La sección
  «Integraciones» gana el botón **«Probar conexión»** y muestra el resultado.
- **El `scope` concedido se hace visible**: la sección «Integraciones» muestra el `scope` que Strava
  concedió y avisa cuando falta `activity:read_all` (reconexión necesaria).
- **Desconectar ya no silencia la revocación**: `POST /api/strava/disconnect` limpia siempre los tokens
  locales y **comprueba** la respuesta de `POST /oauth/revoke`; si la revocación no se confirma,
  devuelve `{connected: false, warning: "<motivo>"}` y la interfaz avisa de que hay que retirar el
  acceso también desde `strava.com/settings/apps`.

## Impact

### Specs modificadas

- `tools/strava`: el requisito de **fallos accionables** incorpora el cuerpo de la respuesta de Strava
  y el estado `Application/Status/Inactive`; el requisito de **estado y desconexión** añade
  `GET /api/strava/check` y el aviso de revocación no confirmada.
- `strava-ui`: el requisito de la sección «Integraciones» añade el `scope` visible, la comprobación
  real de la conexión y el aviso de revocación no confirmada.

### Código

- `src/services/strava.rs`: variante `StravaError::ApplicationInactive`, lectura y parseo del cuerpo de
  error de Strava, `Strava::check`, y `disconnect` que informa del resultado de la revocación.
- `src/handlers/strava.rs` + `src/routes/strava.rs`: `GET /api/strava/check` y `disconnect` con aviso.

### Frontend

- `StravaIntegration.tsx`, `useStrava.ts`, `api/client.ts`, `types/index.ts`: botón «Probar conexión»,
  `scope` visible con aviso y aviso de revocación no confirmada.

### Datos

- Sin migraciones y sin claves nuevas en `settings`: el diagnóstico no persiste estado.

### No-objetivos (fuera de alcance, congelados)

- Añadir `approval_prompt=force` al `authorize` (para forzar la pantalla de consentimiento al
  reconectar). Se valora en un cambio futuro.
- Persistir el último error de Strava en `settings` para mostrarlo sin pulsar nada.
- Widget de gráficas de ritmo/volumen (segunda iteración del Tema 3).
- Sustituir la app OAuth por el MCP oficial de Strava (Valet no habla MCP).
