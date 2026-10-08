## RENAMED Requirements

- FROM: `### Requirement: La configuración del enrutador SHALL vivir en settings, leerse en cada turno y arrancar apagada`
- TO: `### Requirement: La configuración del enrutador SHALL vivir en settings, leerse en cada turno y arrancar encendida`

## MODIFIED Requirements

### Requirement: La configuración del enrutador SHALL vivir en settings, leerse en cada turno y arrancar encendida

La configuración SHALL vivir en la tabla `settings` —`ROUTER_ENABLED` (sembrado como `true`, para que el enrutador arranque encendido; un valor **ausente o ilegible** SHALL caer al default compilado `false`, de modo que sin la clave no se envía nada al clasificador), `ROUTER_MODEL`, `ROUTER_THRESHOLD` (por defecto `0.10`), los overrides por skill `ROUTER_THRESHOLD_<ID>`, `ROUTER_TIMEOUT_MS`, `ROUTER_HISTORY_TURNS` (por defecto `6`) y las claves de criterios y fragmentos por skill—, sembrada y ajustada por migraciones **idempotentes** que SHALL fijar un valor solo cuando siga siendo **exactamente el sembrado**, para no pisar una elección distinta del usuario —aunque un `false` explícito que parta del valor sembrado es indistinguible de él y puede ajustarse **una vez**, como ya hizo la migración de umbrales—, y SHALL leerse en cada turno de modo que editarla surta efecto sin reiniciar. El umbral efectivo de una skill SHALL ser su override si existe y, si no, su umbral por defecto; un valor ausente o ilegible SHALL caer al default registrando un warning. El endpoint y la credencial SHALL proceder del entorno. Los valores por defecto documentados proceden de la medición del arnés sobre el historial real, no de una elección de gusto.

#### Scenario: Por defecto el enrutado está encendido
- **Given** una base de datos recién migrada
- **When** se lee la configuración del enrutador
- **Then** `ROUTER_ENABLED` es `true`
- **And** el enrutador queda habilitado

#### Scenario: Por defecto el enrutado está apagado
- **Given** una tabla `settings` sin la clave `ROUTER_ENABLED`
- **When** se lee la configuración del enrutador
- **Then** el enrutador queda apagado
- **And** la petición lleva todas las herramientas habilitadas

#### Scenario: El umbral por skill se aplica por encima del global
- **Given** `ROUTER_THRESHOLD` en `0.10` y `ROUTER_THRESHOLD_WIDGETS` en `0.20`
- **When** se resuelve el umbral efectivo de cada skill
- **Then** las skills de dominio usan `0.10`
- **And** `widgets` usa `0.20`

#### Scenario: Un umbral ilegible cae al default
- **Given** `ROUTER_THRESHOLD` o un override `ROUTER_THRESHOLD_<ID>` con un valor no numérico o fuera de `[0, 1]`
- **When** se resuelve el umbral efectivo
- **Then** se usa el umbral por defecto correspondiente
- **And** se registra un warning

#### Scenario: Encender surte efecto sin reiniciar
- **Given** el enrutado apagado
- **When** se pone `ROUTER_ENABLED` a `true` desde los ajustes
- **Then** el turno siguiente ya enruta
