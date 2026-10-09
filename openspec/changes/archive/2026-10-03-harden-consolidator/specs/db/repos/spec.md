# Spec Delta: db/repos

## MODIFIED Requirements

### Requirement: Los parámetros de generación SHALL vivir en settings y leerse en cada llamada

Los parámetros de generación de cada rol SHALL vivir en la tabla `settings` (como los prompts y los
mandos de memoria) y SHALL leerse **en cada llamada** al LLM, de modo que cambiarlos surta efecto
sin reiniciar. Habrá cuatro roles —chat, colapso, fichas y consolidador/compresión— y tres claves
por rol: temperatura, razonamiento y tokens máximos. Una migración SHALL sembrarlas con sus
valores iniciales, respetando cualquier valor ya existente (solo rellena si la clave falta o está
vacía).

| clave | rol | qué es | inicial |
|---|---|---|---|
| `GENERATION_CHAT_TEMPERATURE` | chat | temperatura | `0.7` |
| `GENERATION_CHAT_REASONING` | chat | razonamiento | `` (vacío = default del modelo) |
| `GENERATION_CHAT_MAX_TOKENS` | chat | tokens máximos | `4096` |
| `GENERATION_COLLAPSE_TEMPERATURE` | colapso | temperatura | `0.2` |
| `GENERATION_COLLAPSE_REASONING` | colapso | razonamiento | `off` |
| `GENERATION_COLLAPSE_MAX_TOKENS` | colapso | tokens máximos | `1024` |
| `GENERATION_MEMORY_TEMPERATURE` | fichas | temperatura | `0.3` |
| `GENERATION_MEMORY_REASONING` | fichas | razonamiento | `off` |
| `GENERATION_MEMORY_MAX_TOKENS` | fichas | tokens máximos | `1024` |
| `GENERATION_SEMANTIC_TEMPERATURE` | consolidador/compresión | temperatura | `0.1` |
| `GENERATION_SEMANTIC_REASONING` | consolidador/compresión | razonamiento | `off` |
| `GENERATION_SEMANTIC_MAX_TOKENS` | consolidador/compresión | tokens máximos | `2048` |

El campo de razonamiento SHALL codificarse como cadena: vacío ⇒ no se envía el campo `reasoning`
(el modelo decide); `off` ⇒ `ReasoningSpec::Off`; cualquier otro valor ⇒
`ReasoningSpec::Effort(<nivel>)`. Una temperatura o un `max_tokens` no parseables SHALL caer al
default del rol con un warning; un nivel de razonamiento desconocido SHALL caer a `off` con un
warning.

**Given** una base de datos migrada  
**When** se leen las claves de generación  
**Then** `settings` SHALL contener las doce claves con sus valores iniciales  
**And** cualquier valor no vacío ya existente SHALL respetarse

**Given** un rol y sus claves en `settings`  
**When** una llamada al LLM de ese rol construye su `ChatRequest`  
**Then** SHALL leer las claves en ese momento, no al arrancar

#### Scenario: Las doce claves se siembran con sus defaults
**Given** una base de datos recién migrada  
**When** se consulta `settings`  
**Then** existe `GENERATION_CHAT_TEMPERATURE = 0.7`  
**And** existe `GENERATION_SEMANTIC_REASONING = off`  
**And** existe `GENERATION_SEMANTIC_MAX_TOKENS = 2048`  
**And** existen las nueve claves restantes con sus valores iniciales

#### Scenario: Un valor existente se respeta
**Given** `GENERATION_CHAT_TEMPERATURE = 0.9` antes de migrar  
**When** se ejecuta la migración  
**Then** `GENERATION_CHAT_TEMPERATURE` sigue siendo `0.9`

#### Scenario: Un cambio en caliente surte efecto sin reiniciar
**Given** `GENERATION_COLLAPSE_TEMPERATURE = 0.2`  
**When** se actualiza a `0.5` y se repite una llamada de colapso sin reiniciar  
**Then** el `temperature` del `ChatRequest` es `0.5`

#### Scenario: Una temperatura no parseable cae al default con warning
**Given** `GENERATION_MEMORY_TEMPERATURE = "alta"`  
**When** una llamada de fichas lee sus parámetros  
**Then** la temperatura es `0.3`  
**And** se registra un warning

#### Scenario: Un razonamiento desconocido cae a off con warning
**Given** `GENERATION_COLLAPSE_REASONING = "super"`  
**When** una llamada de colapso lee sus parámetros  
**Then** el razonamiento es `Off`  
**And** se registra un warning

#### Scenario: El razonamiento vacío no se envía
**Given** `GENERATION_CHAT_REASONING = ""`  
**When** una llamada del chat lee sus parámetros  
**Then** el `reasoning` del `ChatRequest` es `None`

#### Scenario: Un `low` heredado del consolidador se corrige a `off`
**Given** una base migrada con `GENERATION_SEMANTIC_REASONING = low`  
**When** se aplica la migración de endurecimiento del consolidador  
**Then** `GENERATION_SEMANTIC_REASONING` pasa a `off`

#### Scenario: Un razonamiento del consolidador distinto de `low` se respeta
**Given** una base migrada con `GENERATION_SEMANTIC_REASONING = medium`  
**When** se aplica la migración de endurecimiento del consolidador  
**Then** `GENERATION_SEMANTIC_REASONING` sigue siendo `medium`
