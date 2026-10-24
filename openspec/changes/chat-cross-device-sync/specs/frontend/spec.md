## ADDED Requirements

### Requirement: El chat SHALL mantenerse sincronizado entre clientes en vivo

Resuelta la sesión, el cliente SHALL suscribirse a `GET /api/chat/events` y SHALL fusionar cada evento de mensaje recibido mediante un **upsert por identificador**: si el id ya está en la lista, sustituye la fila; si no, se inserta. La fusión SHALL ser idempotente, de modo que recibir el mismo mensaje dos veces —una por el stream del turno propio y otra por el bus de eventos— SHALL NOT duplicarlo. El cliente SHALL cerrar la suscripción al desmontarse.

#### Scenario: Un mensaje escrito en otro dispositivo aparece sin recargar
**Given** un cliente con el chat abierto
**When** otro cliente envía un mensaje y el turno completa
**Then** el mensaje de usuario aparece en el primer cliente
**And** la respuesta del asistente también aparece
**And** el usuario no ha recargado la página

#### Scenario: El mismo mensaje no se duplica
**Given** un cliente que ha enviado un mensaje y recibe su propia respuesta por el stream del turno
**When** llega además el mismo mensaje por el bus de eventos
**Then** la lista contiene ese mensaje una sola vez

#### Scenario: La suscripción se cierra al salir
**Given** un cliente con la suscripción abierta
**When** el componente que la mantiene se desmonta
**Then** la suscripción se cierra

### Requirement: La lista de mensajes SHALL ordenarse por la pareja del servidor

El cliente SHALL ordenar la lista de mensajes por `created_at` ascendente y, a igualdad, por `id` ascendente, usando **siempre la marca temporal del servidor** para los mensajes persistidos. Un mensaje optimista SHALL permanecer al final de la lista hasta que llegue su fila canónica y lo sustituya. Con esto, el orden tras un replay es idéntico al orden en vivo y dos dispositivos muestran la misma hora para el mismo mensaje.

#### Scenario: Dos dispositivos muestran la misma hora
**Given** un mensaje persistido por el servidor
**When** dos clientes distintos lo muestran
**Then** ambos muestran la misma marca temporal
**And** ninguno la sustituye por la hora local del navegador

#### Scenario: El orden tras un replay es el mismo que en vivo
**Given** un cliente que se reconecta y recibe por replay dos mensajes con la misma marca temporal
**When** los fusiona en la lista
**Then** quedan en el mismo orden relativo que tendrían en vivo

### Requirement: El mensaje de usuario SHALL proponerse con un identificador desde el cliente

Al enviar un mensaje, el cliente SHALL generar un identificador y enviarlo como `client_message_id`, y SHALL usar ese mismo identificador en la burbuja optimista. Cuando llegue la fila canónica con ese id, SHALL sustituir la burbuja optimista en lugar de añadir otra.

#### Scenario: La burbuja optimista se sustituye, no se duplica
**Given** un cliente que envía un mensaje con `client_message_id` `cli-1`
**When** llega la fila canónica del mensaje de usuario con id `cli-1`
**Then** la lista contiene un único mensaje de usuario
**And** ese mensaje lleva la marca temporal del servidor

### Requirement: El cliente SHALL recuperar la sincronía al volver a la pestaña

Cuando la pestaña pasa a ser visible, el cliente SHALL releer los mensajes persistidos posteriores al último cursor conocido y SHALL fusionarlos en la lista, y SHALL reabrir la suscripción si no está abierta. Si la suscripción muere, SHALL reintentarla con retroceso exponencial hasta un máximo de 30 segundos, enviando el último cursor conocido. Un `401` SHALL disparar el evento global de sesión caducada, igual que el resto de la aplicación.

#### Scenario: Un móvil que vuelve del segundo plano se pone al día
**Given** un cliente cuya suscripción dejó de recibir mientras la pestaña estaba oculta
**And** se han persistido mensajes nuevos en ese intervalo
**When** la pestaña vuelve a ser visible
**Then** los mensajes nuevos aparecen en la lista
**And** la suscripción vuelve a estar abierta

#### Scenario: La suscripción se reintenta tras caerse
**Given** una suscripción que termina por un fallo de red
**When** el cliente la reintenta
**Then** envía el último cursor conocido
**And** espera un tiempo creciente entre intentos, sin superar los 30 segundos

#### Scenario: Una sesión caducada no se traga el fallo
**Given** una suscripción que recibe `401`
**When** el cliente lo detecta
**Then** dispara el evento global de sesión caducada
**And** no reintenta en bucle

### Requirement: El cliente SHALL mostrar que hay un turno en curso en otro dispositivo

Al recibir el inicio de un turno, o un estado inicial con el chat ocupado, el cliente SHALL mostrar un indicador de que Valet está respondiendo; al recibir el fin del turno SHALL retirarlo.

#### Scenario: El indicador aparece mientras responde el otro dispositivo
**Given** un cliente con el chat abierto y sin turno propio en curso
**When** otro cliente empieza un turno
**Then** el cliente muestra el indicador de que Valet está respondiendo
**And** al terminar el turno el indicador desaparece

#### Scenario: Un cliente que se conecta a mitad de turno también lo ve
**Given** un turno en curso
**When** un cliente abre el chat
**Then** su estado inicial informa de que el chat está ocupado
**And** el indicador aparece sin esperar a un evento nuevo

### Requirement: Un envío con el turno ocupado SHALL avisarse sin perder el texto

Si el envío recibe `409`, el cliente SHALL mostrar el mensaje de error del servidor y SHALL NOT dejar la burbuja optimista como si el turno hubiera salido, de modo que el texto no se dé por enviado.

#### Scenario: El segundo dispositivo recibe un aviso legible
**Given** un turno en curso iniciado en otro dispositivo
**When** este cliente intenta enviar un mensaje
**Then** el cliente muestra un aviso legible de que el chat está ocupado
**And** el mensaje no queda en la lista como enviado

---
