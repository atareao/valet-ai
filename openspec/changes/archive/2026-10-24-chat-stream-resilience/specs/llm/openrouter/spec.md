## ADDED Requirements

### Requirement: El timeout del cliente SHALL ser configurable por entorno

`OpenRouterConfig` SHALL obtener sus timeouts de variables de entorno, con **120 s** como default: un timeout total para peticiones no-streaming y un timeout de inactividad para streaming. La construcción del cliente SHALL respetar esos valores. Un valor ausente o inválido SHALL caer al default sin abortar el arranque. Ollama SHALL ofrecer las mismas variables para sus timeouts.

#### Scenario: El timeout total viene del entorno
**Given** `OPENROUTER_TIMEOUT_SECS=300`
**When** se construye `OpenRouterConfig` desde el entorno
**Then** el timeout total es 300 s

#### Scenario: El timeout de inactividad viene del entorno
**Given** `OPENROUTER_STREAM_READ_TIMEOUT_SECS=45`
**When** se construye `OpenRouterConfig` desde el entorno
**Then** el timeout de inactividad del streaming es 45 s

#### Scenario: Sin variables, los defaults son 120 s
**Given** ninguna variable de timeout definida
**When** se construye `OpenRouterConfig` desde el entorno
**Then** ambos timeouts son 120 s

#### Scenario: Un valor inválido cae al default
**Given** `OPENROUTER_TIMEOUT_SECS=abc`
**When** se construye `OpenRouterConfig` desde el entorno
**Then** el timeout total es 120 s
**And** el arranque no falla
