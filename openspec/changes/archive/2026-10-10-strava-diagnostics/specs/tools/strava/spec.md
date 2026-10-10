## MODIFIED Requirements

### Requirement: El estado de la conexión SHALL exponerse sin revelar los tokens y la desconexión SHALL revocar en Strava

`GET /api/strava/status` SHALL devolver el estado de la conexión —si está conectada, el atleta y el
`scope`— y SHALL **nunca** incluir los tokens en la respuesta. `GET /api/strava/check` SHALL comprobar
la conexión **contra Strava en el momento**: consulta `GET https://www.strava.com/api/v3/athlete` sin
servirse de la caché corta y SHALL responder **siempre `200`** con `ok`, `athlete_id`, `athlete_name` y
`error` —`ok: true` con el atleta cuando Strava responde, `ok: false` con un mensaje accionable cuando
no—. `POST /api/strava/disconnect` SHALL revocar el acceso en Strava (`POST
https://www.strava.com/oauth/revoke`, autenticado con Basic `client_id:client_secret`) y SHALL borrar
los tokens de `settings` **siempre**, aunque la revocación no se confirme; cuando la revocación no se
confirme SHALL devolver el aviso del fallo junto al estado desconectado. La API de ajustes
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

#### Scenario: La comprobación confirma una conexión sana
- **Given** una integración conectada y una API de Strava que responde `200` a `/athlete`
- **When** se llama a `GET /api/strava/check`
- **Then** la respuesta es `200` con `ok: true`
- **And** incluye el atleta
- **And** no incluye ningún token

#### Scenario: La comprobación reporta el fallo sin romper
- **Given** una integración conectada y una API de Strava que responde `403` con `Application/Status/Inactive`
- **When** se llama a `GET /api/strava/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje indica que la aplicación de Strava está inactiva y cómo reactivarla

#### Scenario: La comprobación consulta Strava en el momento
- **Given** una lectura de `/athlete` ya cacheada hace menos de la ventana de caché corta
- **When** se llama a `GET /api/strava/check`
- **Then** la comprobación consulta Strava de nuevo
- **And** no se sirve de la caché

#### Scenario: Desconectar avisa si la revocación no se confirma
- **Given** una integración conectada y una revocación que falla o no se confirma
- **When** se llama a `POST /api/strava/disconnect`
- **Then** los tokens se borran igualmente de `settings`
- **And** la respuesta indica que no está conectada
- **And** incluye un aviso con el motivo de la revocación no confirmada

### Requirement: Ante un fallo de la conexión o de Strava las herramientas SHALL fallar con un mensaje accionable

Ante un fallo, las herramientas SHALL devolver un **mensaje accionable** y SHALL NOT propagar un error
crudo ni provocar un pánico. Sin conexión o sin configuración, el mensaje SHALL pedir conectar la cuenta
(«conecta tu cuenta de Strava»). Ante un `401`, SHALL intentarse **un refresco y un reintento** una sola
vez y, si persiste, SHALL devolverse un mensaje claro. Ante `403` o `404` SHALL devolverse un mensaje que
**incorpore lo que Strava explica en el cuerpo de la respuesta**: su `message` y sus `errors[]` con
`field` y `code`. Ante `429` SHALL devolverse un mensaje que mencione el **límite** de tasa alcanzado. El
mismo parseo SHALL aplicarse a los fallos del canje y del refresco de tokens.

Cuando el cuerpo de la respuesta de Strava identifique una **aplicación inactiva**
(`{"resource":"Application","field":"Status","code":"Inactive"}`), el mensaje SHALL ser específico y
accionable: SHALL explicar que la aplicación de Strava está inactiva porque la cuenta que la posee
necesita una **suscripción activa** y SHALL indicar dónde reactivarla
(`https://www.strava.com/settings/api`). Cuando el cuerpo no sea JSON, venga vacío o no encaje, SHALL
conservarse el **código HTTP** sin inventar contenido.

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

#### Scenario: Un 403 de aplicación inactiva se explica
- **Given** una respuesta `403` de Strava con `{"message":"Forbidden","errors":[{"resource":"Application","field":"Status","code":"Inactive"}]}`
- **When** se invoca una herramienta de Strava
- **Then** el mensaje explica que la aplicación de Strava está inactiva
- **And** indica la suscripción activa de la cuenta propietaria como causa
- **And** indica que se reactive en `https://www.strava.com/settings/api`

#### Scenario: Un 403 genérico incorpora lo que dice Strava
- **Given** una respuesta `403` de Strava cuyo cuerpo trae un `message` y un `errors[]` distintos de la aplicación inactiva
- **When** se invoca una herramienta de Strava
- **Then** el mensaje incluye el `message` y el `field`/`code` que Strava devuelve
- **And** no se pierde el código HTTP

#### Scenario: Un cuerpo que no es JSON no rompe nada
- **Given** un fallo de Strava con un cuerpo vacío o que no es JSON
- **When** se invoca una herramienta de Strava
- **Then** el mensaje conserva el código HTTP de la respuesta
- **And** no se produce ningún pánico
