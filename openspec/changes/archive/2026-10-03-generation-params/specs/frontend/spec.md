# Spec Delta: frontend

## ADDED Requirements

### Requirement: SettingsDialog SHALL display a Generación tab with the generation knobs

SettingsDialog SHALL display a "Generación" tab with four blocks —Chat, Colapso, Fichas y
Consolidación—, y dentro de cada bloque tres campos: temperatura (numérico), razonamiento
(selector) y tokens máximos (numérico). El selector de razonamiento SHALL ofrecer al menos
`default` (no enviar), `off`, `minimal`, `low`, `medium`, `high`, `xhigh` y `max`. Los valores SHALL
cargarse de `GET /settings` y guardarse con `PUT /settings`, sin rutas nuevas de API. Al ajustarse
en caliente, un cambio guardado SHALL surtir efecto sin reiniciar.

**Given** el SettingsDialog está abierto en la tab "Generación"  
**When** se renderiza  
**Then** muestra cuatro bloques, uno por rol  
**And** cada bloque muestra temperatura, razonamiento y tokens máximos  
**And** cada campo muestra el valor actual cargado de `GET /settings`  
**And** un botón "Guardar" persiste las doce claves vía `PUT /settings`

#### Scenario: La pestaña muestra los cuatro roles y sus tres campos
**Given** el SettingsDialog abierto en la tab "Generación"  
**When** se renderiza  
**Then** existen los bloques Chat, Colapso, Fichas y Consolidación  
**And** cada bloque tiene temperatura, razonamiento y tokens máximos

#### Scenario: Los valores se cargan desde la BD
**Given** `GET /settings` devuelve `GENERATION_CHAT_TEMPERATURE = 0.7` y `GENERATION_SEMANTIC_REASONING = low`  
**When** se abre la tab "Generación"  
**Then** el campo de temperatura del chat muestra `0.7`  
**And** el selector de razonamiento de consolidación muestra `low`

#### Scenario: El selector de razonamiento ofrece las opciones esperadas
**Given** el SettingsDialog abierto en la tab "Generación"  
**When** se abre el selector de razonamiento de cualquier rol  
**Then** ofrece `default`, `off`, `minimal`, `low`, `medium`, `high`, `xhigh` y `max`

#### Scenario: Guardar envía las doce claves y muestra confirmación
**Given** el usuario edita la temperatura del chat  
**When** hace clic en "Guardar"  
**Then** `updateSettings` se llama con las doce claves, las editadas y las demás con su valor actual  
**And** se muestra el mensaje "Ajustes guardados"

#### Scenario: Editar un campo no borra los demás
**Given** el usuario modifica solo los tokens máximos del consolidador  
**When** hace clic en "Guardar"  
**Then** las otras once claves se envían con sus valores actuales sin cambios
