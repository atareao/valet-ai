## ADDED Requirements

### Requirement: Las criteria por defecto del briefing SHALL reconocer el saludo de apertura del día

Las `criteria_true` por defecto de las skills `agenda`, `pendientes`, `entorno` y `widgets` SHALL declarar el **saludo de apertura del día** («buenos días», «¿qué tal?») como caso afirmativo, porque en este asistente un saludo abre el **briefing matutino**: la agenda del día, lo pendiente, la previsión y un panel con los datos. Las skills `recuerdos` y `web` SHALL NOT declararlo. El resto de cada criterio SHALL seguir describiendo su dominio.

#### Scenario: Las cuatro skills del briefing declaran el saludo
- **Given** el catálogo por defecto
- **When** se leen las `criteria_true` de `agenda`, `pendientes`, `entorno` y `widgets`
- **Then** las cuatro declaran el saludo de apertura del día

#### Scenario: Las skills ajenas al briefing no lo declaran
- **Given** el catálogo por defecto
- **When** se leen las `criteria_true` de `recuerdos` y `web`
- **Then** ninguna declara el saludo de apertura del día

## MODIFIED Requirements

### Requirement: La selección SHALL gobernar también el turno sin herramientas

Si ninguna probabilidad alcanza `ROUTER_THRESHOLD`, la selección SHALL ser vacía y el turno SHALL exponer **únicamente el conjunto core**.

#### Scenario: Un turno conversacional no expone herramientas enrutables
- **Given** el enrutador activo y un turno conversacional sin petición de acción (p. ej. «gracias, perfecto»)
- **When** todas las probabilidades quedan por debajo del umbral
- **Then** la selección es vacía
- **And** la petición lleva solo las herramientas del core
