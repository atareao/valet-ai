## ADDED Requirements

### Requirement: El orquestador SHALL preservar el texto parcial cuando el stream falla a mitad

Cuando el stream del modelo falla después de haber emitido fragmentos de texto, el orquestador SHALL incluir en el evento de error enviado al cliente el **texto parcial acumulado** hasta ese punto, en vez de descartarlo. Si el fallo ocurre antes de emitir ningún fragmento, el evento de error SHALL NOT llevar texto parcial. El texto parcial SHALL NOT persistirse en `messages`.

#### Scenario: Fallo tras algunos fragmentos conserva el texto
**Given** un turno cuyo stream emite "Hola, esto es una resp" y después falla
**When** el orquestador emite el evento de error al cliente
**Then** el evento incluye el texto parcial "Hola, esto es una resp"

#### Scenario: Fallo sin fragmentos no lleva texto
**Given** un turno cuyo stream falla antes de emitir ningún fragmento
**When** el orquestador emite el evento de error
**Then** el evento no lleva texto parcial

#### Scenario: El parcial no se persiste
**Given** un turno cuyo stream falla tras emitir fragmentos
**When** el turno termina
**Then** NO se persiste ningún mensaje de asistente con el texto parcial
