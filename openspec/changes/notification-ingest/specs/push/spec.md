## ADDED Requirements

### Requirement: El envío masivo SHALL alcanzar todas las suscripciones y depurar las caducadas

La operación de envío del transporte SHALL aceptar un payload y SHALL leer **todas** las
suscripciones registradas, intentar el envío a cada una y **contabilizar por suscripción** cuatro
contadores: cuántas se intentaron (`attempted`), cuántas aceptó el push service (`accepted`), cuántas
fallaron (`failed`) y cuántas se depuraron (`pruned`). Un resultado `Delivered` SHALL contar como
**aceptada** (`accepted`). Un resultado `Gone` (404/410) SHALL contar como **intentada** y como
**depurada** (`pruned`), SHALL eliminar la suscripción de la base de datos y SHALL NOT contar nunca
como fallida. Un resultado `Failed` —cualquier código distinto de 404/410— y cualquier error de
transporte (`Err(PushError)`) SHALL contar como **intentada** y como **fallida** (`failed`). El fallo
de una suscripción SHALL NOT cortocircuitar el resto: el envío SHALL intentarse con todas. La
operación con **cero suscripciones** SHALL NOT ser un error y SHALL devolver todos los contadores a
cero.

Los registros de fallo SHALL incluir **solo** el **host** del endpoint y el identificador de la
suscripción, y SHALL NOT incluir la URL completa del endpoint, porque un endpoint WebPush es una URL
de capacidad: quien la posee puede enviar avisos a ese dispositivo. La extracción del host SHALL ser
un ayudante **puro**, compartido con el worker de recordatorios, que pase a usarlo en vez de su copia
privada; el ayudante SHALL devolver el host de una URL normal y un marcador cuando el valor no rinda
un host reconocible, sin devolver nunca la cadena original.

#### Scenario: Una mezcla de resultados cuenta cada suscripción por separado

- **Given** tres suscripciones registradas
- **When** sus envíos devuelven `Delivered`, `Gone` y `Failed` respectivamente
- **Then** `attempted` es 3, `accepted` es 1, `pruned` es 1 y `failed` es 1

#### Scenario: Una suscripción caducada se elimina y no cuenta como fallo

- **Given** una suscripción registrada
- **When** su envío devuelve `Gone` (404/410)
- **Then** `attempted` es 1, `pruned` es 1 y `failed` es 0
- **And** la suscripción se elimina de la base de datos

#### Scenario: Sin suscripciones no es un error

- **Given** ninguna suscripción registrada
- **When** se realiza el envío masivo
- **Then** la operación no falla
- **And** todos los contadores son 0

#### Scenario: Un endpoint sin host reconocible no registra la URL

- **Given** una suscripción cuyo endpoint no rinde un host reconocible
- **When** el envío falla
- **Then** el registro NO contiene la URL completa del endpoint
