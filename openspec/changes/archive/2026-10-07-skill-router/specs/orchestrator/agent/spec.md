## ADDED Requirements

### Requirement: El orquestador SHALL enrutar las herramientas por turno antes del bucle ReAct

Con el enrutador activo, la petición al LLM SHALL llevar el subconjunto `core ∪ skills_seleccionadas ∩ habilitadas`, decidido **una sola vez por turno y antes del bucle ReAct**. Con el enrutador apagado o ante cualquier fallo del clasificador, la petición SHALL llevar todas las herramientas habilitadas, de modo que el fallo abierto reproduzca exactamente el comportamiento sin enrutador. Este requisito complementa «La petición al LLM SHALL ofrecer únicamente las herramientas habilitadas», que sigue vigente.

#### Scenario: Con el enrutador activo la petición lleva el subconjunto decidido
- **Given** el enrutador activo y una selección con la skill de agenda
- **When** se construye la petición al LLM
- **Then** las herramientas ofrecidas son el core más las de agenda
- **And** no figuran las de las skills no seleccionadas

#### Scenario: Con el enrutador apagado nada cambia
- **Given** el enrutador apagado
- **When** se construye la petición al LLM
- **Then** se ofrecen todas las herramientas habilitadas

#### Scenario: El fallo del clasificador reproduce el comportamiento sin enrutador
- **Given** el enrutador activo y un clasificador que falla
- **When** se construye la petición al LLM
- **Then** se ofrecen todas las herramientas habilitadas

#### Scenario: La segunda iteración no vuelve a decidir
- **Given** un turno cuya primera iteración produjo una llamada a herramienta
- **When** se construye la petición de la segunda iteración
- **Then** ofrece el mismo subconjunto que la primera
- **And** no se invoca de nuevo al clasificador
