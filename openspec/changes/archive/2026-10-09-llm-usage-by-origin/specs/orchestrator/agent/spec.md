## ADDED Requirements

### Requirement: El orquestador SHALL persistir la llamada al clasificador marcada como del router

Cuando el enrutado haya invocado al clasificador (fuente `Router` o `Error`), el orquestador SHALL insertar en `llm_requests` una fila con `kind='router'` usando `StatsRepo::record_request`, con el modelo del enrutador, los tokens de entrada y salida, el coste, la latencia y el estado (`'success'` o `'error'`). El orquestador SHALL NOT insertar ninguna fila cuando la fuente sea `Disabled` ni `NoRoutableSkills` (no hubo llamada al clasificador). El orquestador SHALL NOT alterar `last_api_call`.

#### Scenario: Un turno enrutado persiste la llamada del clasificador
- **Given** el enrutador activo y una decisión del clasificador
- **When** el orquestador completa el enrutado
- **Then** existe una fila en `llm_requests` con `kind='router'` y el modelo del enrutador
- **And** lleva el coste y la latencia de la decisión

#### Scenario: El fallo del clasificador se persiste como error
- **Given** el enrutador activo y un clasificador que falla (timeout o error HTTP)
- **When** el orquestador completa el enrutado
- **Then** existe una fila con `kind='router'` y `status='error'`

#### Scenario: Un turno que no llama al clasificador no persiste nada
- **Given** el enrutador apagado o sin skills enrutables
- **When** el orquestador completa el enrutado
- **Then** no se inserta ninguna fila con `kind='router'`

#### Scenario: La última llamada sigue siendo la del chat
- **Given** un turno enrutado
- **When** se persiste la llamada del clasificador
- **Then** `last_api_call` no cambia
