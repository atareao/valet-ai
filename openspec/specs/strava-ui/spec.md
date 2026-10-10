# strava-ui Specification

## Purpose

La cara visible de la integración con Strava en el diálogo de ajustes: introducir las credenciales, conectar la cuenta con el botón oficial, ver si está conectada —y como qué atleta— y desconectarla, sin exponer jamás los tokens.

## Requirements

### Requirement: Los ajustes SHALL permitir conectar, ver el estado y desconectar la cuenta de Strava

El diálogo de ajustes SHALL incluir una sección **«Integraciones»** para la cuenta de Strava, con los
campos de `client_id` y `client_secret`, el **botón oficial «Connect with Strava»** que inicia el flujo
OAuth, el **estado** de la conexión (conectada como \<atleta\> o no conectada) y la acción de
**desconectar**. Los tokens **NO** SHALL mostrarse en la interfaz en ningún caso.

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
