## ADDED Requirements

### Requirement: El resumen del ciclo SHALL contabilizar el resultado por suscripción

`TickOutcome` SHALL contar, además de sus contadores actuales de recordatorios (`delivered`,
`skipped_old`, `pruned`, `unparseable`), el resultado de cada envío individual: cuántos envíos se
intentaron (`attempted`), cuántos aceptó el push service (`accepted`) y cuántos fallaron (`failed`).
`failed` SHALL agrupar **todo lo que no fue aceptado ni depurado**: los rechazos del push service
(`Failed`, cualquier código que no sea 404/410) y los errores de transporte. Un envío que termina
en `Err(PushError)` —fallo de red, timeout o respuesta ilegible— SHALL contar como **intentado**
(`attempted`) y como **fallido** (`failed`), porque desde la perspectiva de la observabilidad es un
envío que **no llegó**, exactamente igual que un 5xx, y SHALL NOT desaparecer del recuento ni
contarse como aceptado. Una suscripción depurada por `Gone` (404/410) SHALL contar como
**intentada** y como **depurada** (`pruned`), y SHALL NOT contar como fallida. La decisión de
registrar el resumen SHALL vivir en un predicado puro sobre los contadores, `should_log_summary`,
que SHALL ser verdadero cuando **cualquiera** de ellos sea distinto de cero —incluidos `unparseable`
y `failed`—.

#### Scenario: Una mezcla de resultados cuenta cada suscripción por separado

- **Given** un recordatorio vencido y 3 suscripciones registradas
- **When** el sender devuelve `Delivered`, `Gone` y `Failed` respectivamente
- **Then** `attempted` es 3, `accepted` es 1, `pruned` es 1 y `failed` es 1
- **And** `delivered` es 1, porque al menos una entrega fue aceptada y el recordatorio se marca

#### Scenario: Un fallo de transporte cuenta como intentado y fallido

- **Given** un recordatorio vencido y una suscripción registrada
- **When** el sender devuelve `Err(PushError)` —un fallo de red, un timeout o una respuesta ilegible—
- **Then** `attempted` es 1, `failed` es 1, `accepted` es 0 y `pruned` es 0
- **And** `delivered` es 0 y el recordatorio NO se marca
- **And** `should_log_summary()` es verdadero

#### Scenario: Todos los envíos fallan pide resumen aunque no haya entrega

- **Given** un recordatorio vencido y 2 suscripciones registradas
- **When** el sender devuelve `Failed` en ambas
- **Then** `failed` es 2, `accepted` es 0 y `delivered` es 0
- **And** el recordatorio NO se marca
- **And** `should_log_summary()` es verdadero

#### Scenario: Un ciclo sin nada que hacer no registra resumen

- **Given** ninguna suscripción registrada y ningún recordatorio vencido
- **When** el worker ejecuta un ciclo
- **Then** todos los contadores son 0
- **And** `should_log_summary()` es falso

#### Scenario: Un datetime no parseable se cuenta y pide resumen

- **Given** un recordatorio pendiente sin notificar cuyo `datetime` no parsea
- **When** el worker ejecuta un ciclo
- **Then** `unparseable` es 1 y `delivered` es 0
- **And** el recordatorio NO se marca
- **And** `should_log_summary()` es verdadero

#### Scenario: Sin suscripciones no hay envíos intentados

- **Given** un recordatorio vencido y ninguna suscripción registrada
- **When** el worker ejecuta un ciclo
- **Then** `attempted` es 0 y `accepted` es 0
- **And** `delivered` es 0 y el recordatorio NO se marca

### Requirement: Los registros SHALL NOT exponer el endpoint completo de una suscripción

Un endpoint de suscripción es una URL de capacidad. Los registros del `ReminderPushWorker` SHALL
incluir solo el **host** del endpoint y el identificador de la suscripción, y SHALL NOT incluir la
URL completa del endpoint. La extracción del host SHALL ser un ayudante puro que devuelva el host de
una URL normal y un marcador (`(endpoint no parseable)`) cuando el valor no rinda un host
reconocible, sin devolver nunca la cadena original.

#### Scenario: Un fallo de envío redacta el endpoint

- **Given** una suscripción cuyo endpoint es `https://fcm.googleapis.com/fcm/send/<identificador-largo>`
- **When** el sender devuelve `Failed`
- **Then** el registro incluye `fcm.googleapis.com`
- **And** el registro NO incluye la ruta `/fcm/send/<identificador-largo>`

#### Scenario: El ayudante devuelve el host de una URL normal

- **Given** el endpoint `https://fcm.googleapis.com/fcm/send/abc`
- **When** se extrae el host
- **Then** devuelve `fcm.googleapis.com`

#### Scenario: El ayudante marca lo que no puede interpretar

- **Given** un endpoint sin host reconocible, como una cadena vacía
- **When** se extrae el host
- **Then** devuelve el marcador `(endpoint no parseable)`
- **And** NO devuelve la cadena original
