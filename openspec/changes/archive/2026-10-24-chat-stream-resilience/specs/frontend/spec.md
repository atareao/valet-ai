## ADDED Requirements

### Requirement: El chat SHALL mantener visible el texto parcial interrumpido

Cuando un turno falla después de haber mostrado texto del asistente, el cliente SHALL **mantener** ese texto en la burbuja del asistente y SHALL mostrar un aviso claro de que la respuesta se interrumpió. El cliente SHALL NOT borrar el texto parcial recibido al llegar el evento de error.

#### Scenario: El texto parcial no desaparece
**Given** un cliente que está mostrando la respuesta del asistente escribiéndose
**When** llega un evento de error que incluye el texto parcial
**Then** la burbuja conserva el texto parcial
**And** muestra un aviso de respuesta interrumpida

#### Scenario: Error sin texto parcial no deja una burbuja vacía
**Given** un turno que falla antes de mostrar texto
**When** llega el evento de error sin texto parcial
**Then** se muestra el aviso de error
**And** no queda una burbuja de asistente vacía
