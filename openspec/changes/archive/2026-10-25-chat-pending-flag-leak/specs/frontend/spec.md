## MODIFIED Requirements

### Requirement: La lista de mensajes SHALL ordenarse por la pareja del servidor

El cliente SHALL ordenar la lista de mensajes por `created_at` ascendente y, a igualdad, por `id` ascendente, usando **siempre la marca temporal del servidor** para los mensajes persistidos. Un mensaje optimista SHALL permanecer al final de la lista hasta que llegue su fila canónica y lo sustituya. La condición de «optimista» SHALL viajar **en el propio mensaje** —un campo interno que no viaja al servidor— y SHALL NOT residir en un estado aparte indexado por identificador. Una fila canónica SHALL NOT conservar la marca de optimista, de modo que el orden por `(created_at, id)` prevalezca en cuanto exista la fila del servidor. Con esto, el orden tras un replay es idéntico al orden en vivo y dos dispositivos muestran la misma hora para el mismo mensaje.

#### Scenario: Dos dispositivos muestran la misma hora
**Given** un mensaje persistido por el servidor
**When** dos clientes distintos lo muestran
**Then** ambos muestran la misma marca temporal
**And** ninguno la sustituye por la hora local del navegador

#### Scenario: El orden tras un replay es el mismo que en vivo
**Given** un cliente que se reconecta y recibe por replay dos mensajes con la misma marca temporal
**When** los fusiona en la lista
**Then** quedan en el mismo orden relativo que tendrían en vivo

#### Scenario: La fila canónica de un mensaje llega antes que el cierre del turno
**Given** un cliente que ha enviado un mensaje y muestra su burbuja optimista
**When** la fila canónica del asistente llega por el bus de eventos antes que el evento `done` del stream
**And** después llega el `done` con el mismo identificador
**Then** el mensaje del asistente se ordena por su marca temporal del servidor
**And** al enviar un mensaje posterior, el asistente anterior permanece por encima del mensaje nuevo

#### Scenario: El estado de optimista no sobrevive a la fila canónica
**Given** un mensaje optimista marcado como pendiente
**When** llega su fila canónica con el mismo identificador
**Then** el mensaje deja de estar marcado como pendiente
**And** vuelve a su posición dictada por la pareja `(created_at, id)`
