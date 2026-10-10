## ADDED Requirements

### Requirement: La conexión con Strava SHALL establecerse por OAuth2 en nombre del atleta y en solo lectura

Valet SHALL conectar con Strava por **OAuth2 en nombre de un único atleta** (Single Player Mode) y en
**solo lectura**. La conexión SHALL comenzar en `GET /api/strava/authorize`, que redirige al usuario
a `GET https://www.strava.com/oauth/authorize` con los parámetros `client_id`, `redirect_uri`,
`response_type=code`, `state` y `scope`; el `code` devuelto SHALL canjearse en el callback
`GET /api/strava/callback` contra `POST https://www.strava.com/oauth/token` con
`grant_type=authorization_code`. El `scope` solicitado SHALL ser `read,activity:read_all`; Valet SHALL
NOT solicitar `activity:write` ni realizar ninguna escritura en Strava. El `state` SHALL generarse por
intento y SHALL verificarse al recibir el callback: un `state` inválido o ausente SHALL rechazar el
canje. Una respuesta con `error=access_denied` SHALL NOT guardar ningún token.

**Given** la aplicación Valet con la integración de Strava sin conectar
**When** el usuario inicia la conexión
**Then** SHALL redirigirse a `https://www.strava.com/oauth/authorize` con `client_id`, `redirect_uri`, `response_type=code`, `state` y `scope`
**And** el `scope` SHALL ser `read,activity:read_all`
**And** NO SHALL incluirse `activity:write`

#### Scenario: La conexión es de solo lectura
- **Given** una petición de conexión iniciada
- **When** se inspecciona el `scope` solicitado a Strava
- **Then** es `read,activity:read_all`
- **And** no incluye `activity:write` ni ningún permiso de escritura

#### Scenario: Un state inválido rechaza el canje
- **Given** un callback con un `state` ausente o distinto del generado en el intento
- **When** se procesa el callback
- **Then** el canje del `code` NO se realiza
- **And** no se guarda ningún token

#### Scenario: La denegación no deja rastro
- **Given** un callback con `error=access_denied`
- **When** se procesa el callback
- **Then** no se guarda ningún token
- **And** la conexión sigue figurando como no conectada

### Requirement: Los tokens SHALL persistir en settings y el refresco SHALL rotar el refresh token sin perderlo

Los tokens de Strava SHALL persistir en la tabla `settings` bajo las claves `strava_client_id`
(con respaldo en la variable de entorno `STRAVA_CLIENT_ID`), `strava_client_secret` (respaldo
`STRAVA_CLIENT_SECRET`), `strava_refresh_token`, `strava_access_token`, `strava_expires_at`,
`strava_athlete_id` y `strava_scope`. El access token de Strava SHALL durar **6 horas**. El refresco
SHALL usar `grant_type=refresh_token` contra `POST https://www.strava.com/oauth/token`. Como Strava
**devuelve un refresh token nuevo que invalida al anterior**, el refresh token **devuelto SHALL
persistirse en cada refresco**, sustituyendo al viejo, de modo que perderlo no desconecte la
integración. SHALL existir **un único punto de refresco por atleta** (lock) para que dos turnos
concurrentes no se pisen, y el access token SHALL reutilizarse mientras siga vigente con más de **1
hora** de margen.

**Given** una integración conectada cuyos tokens viven en `settings`
**When** se necesita un access token válido
**Then** SHALL usarse el access token vigente si aún le quedan más de 1 hora
**And** si no, SHALL refrescarse con `grant_type=refresh_token`
**And** el refresh token devuelto SHALL guardarse sustituyendo al anterior

#### Scenario: El refresh token nuevo sustituye al viejo
- **Given** una integración con un `strava_refresh_token` guardado
- **When** se realiza un refresco
- **Then** el `strava_refresh_token` devuelto por Strava se persiste en `settings`
- **And** el valor anterior queda reemplazado

#### Scenario: Dos refrescos concurrentes no se pisan
- **Given** dos turnos que necesitan refrescar al mismo tiempo el mismo atleta
- **When** ambos intentan refrescar
- **Then** existe un único punto de refresco por atleta
- **And** el refresh token final persistido es el último devuelto
- **And** ninguno de los dos turnos pierde la conexión

#### Scenario: Un access token caducado se refresca antes de llamar
- **Given** un `strava_expires_at` ya vencido o con menos de 1 hora de margen
- **When** una herramienta va a consultar la API
- **Then** se refresca el access token antes de la llamada
- **And** la consulta se realiza con el token nuevo

### Requirement: El estado de la conexión SHALL exponerse sin revelar los tokens y la desconexión SHALL revocar en Strava

`GET /api/strava/status` SHALL devolver el estado de la conexión —si está conectada, el atleta y el
`scope`— y SHALL **nunca** incluir los tokens en la respuesta. `POST /api/strava/disconnect` SHALL
revocar el acceso en Strava (`POST https://www.strava.com/oauth/revoke`, autenticado con Basic
`client_id:client_secret`) y SHALL borrar los tokens de `settings`. La API de ajustes
(`GET`/`PUT /api/settings`) SHALL NOT exponer las claves `strava_access_token` ni
`strava_refresh_token`, ni SHALL aceptar su escritura: son material sensible gestionado por el flujo
OAuth y SHALL permanecer fuera del alcance de un cliente.

**Given** la aplicación Valet con la integración de Strava
**When** se consulta `GET /api/strava/status`
**Then** SHALL devolver si está conectada, el atleta y el `scope`
**And** NO SHALL incluir `strava_access_token` ni `strava_refresh_token`

#### Scenario: El estado no expone los tokens
- **Given** una integración conectada en `settings`
- **When** se consulta `GET /api/strava/status`
- **Then** la respuesta indica que está conectada y el atleta
- **And** no contiene el access token ni el refresh token

#### Scenario: Los tokens no viajan en los ajustes
- **Given** una integración conectada cuyos tokens viven en `settings`
- **When** se consulta `GET /api/settings` o se envía `PUT /api/settings` con las claves de tokens
- **Then** la respuesta de `GET /api/settings` no contiene `strava_access_token` ni `strava_refresh_token`
- **And** `PUT /api/settings` ignora esas claves y no modifica sus valores

#### Scenario: Desconectar revoca y limpia
- **Given** una integración conectada
- **When** se llama a `POST /api/strava/disconnect`
- **Then** se revoca el acceso en Strava
- **And** los tokens de Strava se borran de `settings`
- **And** el estado pasa a no conectada

### Requirement: Las herramientas de consulta SHALL leer la actividad del atleta en vivo

Las cuatro herramientas de la skill `running` SHALL leer la actividad del atleta **en vivo** desde la
API de Strava, todas de **solo lectura** y devolviendo un **resumen útil** (no volcados crudos):

- `strava_recent_activities` — actividades recientes del atleta, con filtros `before`, `after`,
  `page`, `per_page` y filtro de deporte (`Run`, `TrailRun`, `VirtualRun`).
- `strava_activity_detail` — el detalle de una actividad, incluyendo sus vueltas (*laps*) y *splits*.
- `strava_activity_streams` — las series temporales de una actividad, seleccionables con `keys` y
  `key_by_type` (ritmo, frecuencia cardiaca, cadencia, altitud).
- `strava_athlete_stats` — el perfil del atleta y sus totales (de año y recientes).

**Given** una integración conectada y el cliente HTTP de Strava
**When** el modelo invoca una de las cuatro herramientas
**Then** SHALL consultarse la API de Strava en vivo
**And** SHALL devolverse un resumen útil de la actividad o del atleta
**And** la operación SHALL ser de solo lectura

#### Scenario: strava_recent_activities lista las salidas recientes
- **Given** una integración conectada
- **When** se invoca `strava_recent_activities` con `per_page` y un filtro de deporte
- **Then** devuelve un resumen de las actividades recientes que casan con el filtro
- **And** respeta `before`, `after`, `page` y `per_page`

#### Scenario: strava_activity_detail detalla una actividad con vueltas y splits
- **Given** una integración conectada y un id de actividad
- **When** se invoca `strava_activity_detail`
- **Then** devuelve un resumen de la actividad
- **And** incluye sus vueltas y sus splits

#### Scenario: strava_activity_streams devuelve las series pedidas
- **Given** una integración conectada y un id de actividad
- **When** se invoca `strava_activity_streams` con `keys` y `key_by_type`
- **Then** devuelve las series temporales solicitadas (ritmo, FC, cadencia y/o altitud)

#### Scenario: strava_athlete_stats resume el perfil y los totales
- **Given** una integración conectada
- **When** se invoca `strava_athlete_stats`
- **Then** devuelve el perfil del atleta y sus totales de año y recientes

### Requirement: Ante un fallo de la conexión o de Strava las herramientas SHALL fallar con un mensaje accionable

Ante un fallo, las herramientas SHALL devolver un **mensaje accionable** y SHALL NOT propagar un error
crudo ni provocar un pánico. Sin conexión o sin configuración, el mensaje SHALL pedir conectar la
cuenta («conecta tu cuenta de Strava»). Ante un `401`, SHALL intentarse **un refresco y un reintento**
una sola vez y, si persiste, SHALL devolverse un mensaje claro. Ante `403` o `404` SHALL devolverse un
mensaje claro. Ante `429` SHALL devolverse un mensaje que mencione el **límite** de tasa alcanzado.

**Given** una invocación de una herramienta de Strava
**When** la conexión o la API de Strava falla
**Then** SHALL devolverse un mensaje accionable
**And** NO SHALL propagarse un error crudo ni producirse un pánico

#### Scenario: Sin conexión
- **Given** la integración de Strava sin conectar o sin `client_id`/`client_secret`
- **When** se invoca una herramienta de Strava
- **Then** devuelve un mensaje que pide conectar la cuenta de Strava

#### Scenario: Límite excedido
- **Given** una respuesta `429` de la API de Strava
- **When** se invoca una herramienta de Strava
- **Then** devuelve un mensaje que menciona el límite de tasa alcanzado

### Requirement: El consumo SHALL respetar los límites de tasa de Strava

El consumo de la API de Strava SHALL respetar sus límites de tasa: las lecturas SHALL servirse desde
una **caché corta** cuando se repiten dentro de su ventana, SHALL NOT hacer **polling**, SHALL
atenderse las cabeceras `X-RateLimit-*` y SHALL retrocederse ante un `429`.

**Given** una lectura de Strava ya realizada hace poco
**When** se repite la misma lectura dentro de la ventana de caché
**Then** SHALL servirse desde la caché
**And** NO SHALL realizarse una nueva petición de red

#### Scenario: Una consulta repetida dentro de la ventana de caché no sale a la red
- **Given** una consulta de Strava cuyo resultado está en caché y dentro de su ventana
- **When** se repite la misma consulta
- **Then** el resultado se sirve desde la caché
- **And** no se realiza ninguna petición de red
