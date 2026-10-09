# Spec Delta: persistent-memory

## MODIFIED Requirements

### Requirement: El estado SHALL respetar un presupuesto de tokens con un techo absoluto

El estado persistente SHALL caber en un presupuesto máximo de tokens, medido sobre su forma
inyectada —el JSON minificado— y leído de `settings.PERSISTENT_MEMORY_BUDGET_TOKENS` (por
defecto 800). Si el estado consolidado lo supera, el worker SHALL solicitar una **única**
compresión al modelo del consolidador. SHALL existir además un **techo absoluto** igual al
doble del presupuesto. Si tras la compresión el estado cabe bajo el techo pero sigue
superando el presupuesto, SHALL almacenarse y registrarse un aviso. Si aun así supera el
techo, la escritura SHALL rechazarse y SHALL conservarse el estado anterior, registrando un
aviso. Si la compresión falla o no produce un estado válido, el worker SHALL evaluar el
estado sin comprimir contra el techo: si cabe, SHALL almacenarse con un aviso; si lo supera,
SHALL conservarse el estado anterior con un aviso. En ningún caso un problema de tamaño SHALL
abortar la pasada. Rust SHALL NOT truncar ni descartar contenido en silencio, y el estado
almacenado SHALL ser siempre JSON válido.

**Given** un estado consolidado y el presupuesto configurado  
**When** el estado cabe en el presupuesto  
**Then** SHALL almacenarse tal cual, sin compresión  
**When** supera el presupuesto  
**Then** SHALL intentarse una única compresión con el LLM  
**And** si el resultado cabe bajo el techo, SHALL almacenarse  
**And** si el resultado supera el techo, SHALL rechazarse la escritura, conservarse el estado anterior y registrarse un aviso

#### Scenario: Dentro del presupuesto no se comprime
**Given** un consolidado que cabe en el presupuesto  
**When** se aplica el control de tamaño  
**Then** NO se solicita ninguna compresión  
**And** el estado se almacena tal cual

#### Scenario: La compresión lo ajusta al presupuesto
**Given** un consolidado que supera el presupuesto  
**When** la compresión del LLM lo deja dentro del presupuesto  
**Then** el estado comprimido se almacena  
**And** no se registra ningún aviso de tamaño

#### Scenario: Superar el techo conserva el estado anterior
**Given** un consolidado que supera el presupuesto y una compresión que no baja del techo absoluto  
**When** se aplica el control de tamaño  
**Then** la escritura se rechaza  
**And** el estado anterior permanece intacto  
**And** se registra un aviso

#### Scenario: La compresión fallida degrada sin abortar
**Given** un consolidado que supera el presupuesto y una compresión que falla  
**When** se aplica el control de tamaño  
**Then** la pasada NO se aborta  
**And** si el estado sin comprimir cabe bajo el techo, se almacena con un aviso  
**And** si lo supera, se conserva el estado anterior con un aviso

## ADDED Requirements

### Requirement: El prompt del consolidador sembrado SHALL estructurar el perfil por taxonomía y evitar redundancia

El prompt de consolidación sembrado por migración SHALL guiar al LLM a estructurar `user_profile`
por secciones estables (`identity`, `preferences_and_tastes` con `communication_style`,
`technology_and_tools`, `lifestyle_and_leisure` y `dislikes_and_dealbreakers`,
`lifestyle_and_routines`, `productivity_and_workflow`, `interests_and_knowledge`,
`relationships_and_entities`), incluir **solo** las secciones con contenido, no inferir datos no
afirmados, y reservar `system_rules` para instrucciones explícitas dirigidas al asistente, sin
duplicar datos que ya estén en `user_profile`. SHALL conservar los placeholders
`{{ ESTADO_ACTUAL }}` y `{{ BLOQUE_DE_MENSAJES }}` y el marcador `consolidador de memoria
persistente`.

#### Scenario: El prompt sembrado incluye la taxonomía
**Given** una base de datos recién migrada  
**When** se lee `settings.consolidator_prompt`  
**Then** contiene `preferences_and_tastes` y `dislikes_and_dealbreakers`  
**And** contiene los placeholders `{{ ESTADO_ACTUAL }}` y `{{ BLOQUE_DE_MENSAJES }}`  
**And** contiene el marcador `consolidador de memoria persistente`

#### Scenario: El prompt sembrado prohíbe duplicar e inventar
**Given** una base de datos recién migrada  
**When** se lee `settings.consolidator_prompt`  
**Then** instruye a no repetir en `system_rules` los datos de perfil  
**And** instruye a no inferir datos no afirmados

#### Scenario: Un prompt personalizado se conserva
**Given** una base migrada con `consolidator_prompt` distinto del default anterior  
**When** se aplica la migración de endurecimiento del consolidador  
**Then** `consolidator_prompt` se conserva sin cambios
