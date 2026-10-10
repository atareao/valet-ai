# strava-ui Specification

## Purpose

La cara visible de la integración con Strava en el diálogo de ajustes: introducir las credenciales, conectar la cuenta con el botón oficial, ver si está conectada —y como qué atleta— y desconectarla, sin exponer jamás los tokens.

## Requirements

### Requirement: Los ajustes SHALL permitir conectar, ver el estado y desconectar la cuenta de Strava

El diálogo de ajustes SHALL incluir una sección **«Integraciones»** para la cuenta de Strava, con los
campos de `client_id` y `client_secret`, el **botón oficial «Connect with Strava»** que inicia el flujo
OAuth, el **estado** de la conexión (conectada como \<atleta\> o no conectada), el **`scope` que Strava
concedió** y la acción de **desconectar**. Los tokens **NO** SHALL mostrarse en la interfaz en ningún
caso. Cuando el `scope` concedido no incluya `activity:read_all`, la sección SHALL avisar de que hay que
volver a conectar la cuenta para concederlo. La sección SHALL ofrecer además **«Probar conexión»**, que
comprueba la conexión contra Strava y muestra el resultado: la confirmación con el atleta cuando va bien,
o el **mensaje accionable** que devuelva el servidor cuando no. Si la desconexión no confirma la
revocación en Strava, la sección SHALL mostrar el aviso con el motivo.

**Given** el diálogo de ajustes abierto
**When** se muestra la sección «Integraciones»
**Then** ofrece los campos `client_id` y `client_secret`
**And** ofrece el botón «Connect with Strava»
**And** muestra el estado de la conexión
**And** NO muestra los tokens

#### Scenario: Conectar inicia el flujo OAuth
- **Given** la sección «Integraciones» con `client_id` y `client_secret` informados y sin conectar
- **When** el usuario pulsa «Connect with Strava»
- **Then** se inicia el flujo OAuth de Strava

#### Scenario: Desconectar revoca y limpia
- **Given** una cuenta de Strava conectada
- **When** el usuario pulsa desconectar
- **Then** se revoca el acceso en Strava y se limpian los tokens
- **And** el estado pasa a no conectada

#### Scenario: Los tokens no se exponen
- **Given** una cuenta de Strava conectada
- **When** se muestra la sección «Integraciones»
- **Then** la interfaz no muestra el access token ni el refresh token

#### Scenario: El scope concedido se muestra
- **Given** una cuenta conectada cuyo `scope` concedido es `read,activity:read_all`
- **When** se muestra la sección «Integraciones»
- **Then** la sección muestra el `scope` concedido
- **And** no avisa de permisos que falten

#### Scenario: Un scope incompleto se avisa
- **Given** una cuenta conectada cuyo `scope` concedido no incluye `activity:read_all`
- **When** se muestra la sección «Integraciones»
- **Then** la sección avisa de que hay que volver a conectar para conceder `activity:read_all`

#### Scenario: La comprobación confirma la conexión
- **Given** una sección «Integraciones» con una cuenta conectada y una comprobación que responde `ok: true`
- **When** el usuario pulsa «Probar conexión»
- **Then** la sección muestra la confirmación con el atleta

#### Scenario: La comprobación muestra el fallo accionable
- **Given** una sección «Integraciones» con una cuenta conectada y una comprobación que responde `ok: false` con un mensaje
- **When** el usuario pulsa «Probar conexión»
- **Then** la sección muestra ese mensaje como aviso
- **And** el estado de la conexión no se altera

#### Scenario: La desconexión avisa si la revocación no se confirma
- **Given** una cuenta conectada y una desconexión que devuelve un aviso de revocación no confirmada
- **When** el usuario pulsa desconectar
- **Then** la sección muestra el aviso de que hay que retirar el acceso también desde `https://www.strava.com/settings/apps`
