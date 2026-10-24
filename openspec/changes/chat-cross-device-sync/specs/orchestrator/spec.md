## ADDED Requirements

### Requirement: El orquestador SHALL publicar en el bus cada mensaje que persiste

Tras persistir el mensaje de usuario y tras persistir el mensaje del asistente, el orquestador SHALL publicar en el bus de eventos del chat un evento con la **fila persistida completa** del mensaje (identificador, rol, contenido, marca temporal del servidor, ubicación, `tools_used` y widgets). La publicación SHALL ocurrir **después** de la escritura en base de datos, de modo que cualquier cliente que reaccione al evento pueda leer la misma fila. El orquestador SHALL NOT publicar fragmentos de texto del turno en curso.

#### Scenario: El mensaje de usuario se publica tras persistirse
**Given** un turno que empieza
**When** el orquestador persiste el mensaje de usuario
**Then** publica un evento con la fila persistida de ese mensaje
**And** el evento lleva la marca temporal que quedó en la base de datos

#### Scenario: El mensaje del asistente se publica tras persistirse
**Given** un turno que completa con respuesta
**When** el orquestador persiste el mensaje del asistente
**Then** publica un evento con la fila persistida de ese mensaje
**And** el evento incluye `tools_used` cuando el turno usó herramientas

#### Scenario: Un turno que falla no publica un mensaje del asistente inexistente
**Given** un turno que termina con error del proveedor
**When** el orquestador no llega a persistir el mensaje del asistente
**Then** no publica ningún evento de mensaje del asistente

#### Scenario: Los fragmentos del turno no viajan por el bus
**Given** un turno que produce varios fragmentos de texto
**When** el orquestador los emite al cliente del turno
**Then** publica en el bus únicamente los mensajes que persiste
**And** ningún evento del bus contiene el texto parcial del turno

---
