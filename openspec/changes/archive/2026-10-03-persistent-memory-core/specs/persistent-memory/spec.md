# Spec Delta: persistent-memory

## Purpose

Capa C de Valet: el estado persistente del usuario —perfil (hechos, preferencias e
intereses) y reglas de sistema— guardado como una única fila JSON versionada, acotado por
un presupuesto de tokens y consolidado por un LLM a partir de la conversación.

## ADDED Requirements

### Requirement: La memoria persistente SHALL almacenarse como una única fila JSON versionada

La Capa C SHALL almacenarse en la tabla `persistent_memory` como una **única fila lógica**
identificada por `id = 'global_state'`, con un `payload` TEXT que contenga un objeto JSON y
un `updated_at`. El repositorio SHALL exponer lectura y escritura de esa fila. La
**ausencia** de fila SHALL equivaler a un estado vacío, no a un error.

**Given** una base de datos migrada  
**When** se lee la Capa C  
**Then** SHALL devolverse el `payload` de la fila `'global_state'` si existe  
**And** SHALL devolverse ausencia de estado, sin error, si la fila no existe  
**And** la lectura SHALL NOT crear la fila

#### Scenario: Estado inexistente se lee como vacío
**Given** una base de datos migrada sin fila en `persistent_memory`  
**When** se lee la Capa C  
**Then** el resultado es «sin estado»  
**And** no se ha insertado ninguna fila

#### Scenario: El upsert crea y luego actualiza la misma fila
**Given** una base de datos migrada sin fila en `persistent_memory`  
**When** se escribe un estado y después otro distinto  
**Then** existe exactamente una fila con `id = 'global_state'`  
**And** su `payload` es el último escrito  
**And** su `updated_at` refleja la última escritura

### Requirement: El payload SHALL tener forma fija y versión

El payload SHALL ser un objeto JSON cuyas claves de **primer nivel** pertenezcan al conjunto
permitido: `schema_version` (entero), `user_profile` (objeto) y `system_rules` (array de
cadenas). `user_profile` SHALL agrupar los hechos, las preferencias y los intereses del
usuario; `system_rules` SHALL agrupar las instrucciones de comportamiento. `schema_version`
SHALL valer `1`. Un payload con `schema_version` ausente o distinta de `1` SHALL rechazarse,
conservando el estado anterior. Las claves de primer nivel fuera del conjunto permitido SHALL
descartarse; el conjunto de secciones SHALL ser fijo, de modo que el LLM SHALL NOT añadir
secciones nuevas.

**Given** un payload candidato  
**When** se valida contra el esquema  
**Then** SHALL aceptarse solo si `schema_version` es `1` y su forma es la permitida  
**And** SHALL rechazarse sin escribir nada si la versión falta o no es `1`  
**And** las claves de primer nivel no permitidas SHALL eliminarse del estado aceptado

#### Scenario: Payload válido de versión 1
**Given** un payload con `schema_version = 1`, `user_profile` y `system_rules`  
**When** se valida  
**Then** el payload es aceptado sin cambios de forma

#### Scenario: Versión distinta conserva el estado anterior
**Given** un estado persistente previo y un payload con `schema_version = 2`  
**When** se valida el payload candidato  
**Then** el candidato se rechaza  
**And** el estado persistente anterior permanece intacto

#### Scenario: Claves desconocidas se descartan
**Given** un payload de versión 1 con una clave de primer nivel `"scratchpad"` no permitida  
**When** se valida  
**Then** el estado aceptado no contiene `"scratchpad"`  
**And** el resto del payload se conserva

### Requirement: La marca temporal del estado SHALL escribirla Rust

El `updated_at` de la fila `persistent_memory` SHALL escribirlo Rust, comparando el contenido
del `payload` —excluida la propia marca temporal— con el del estado anterior. Si el contenido
no ha cambiado, SHALL conservarse el `updated_at` previo; si cambió o el estado es nuevo,
SHALL fijarse al instante de la consolidación. El valor de fecha devuelto por el LLM SHALL
ignorarse.

**Given** un estado persistente anterior y un payload consolidado  
**When** el contenido —sin la marca temporal— no ha cambiado  
**Then** SHALL conservarse el `updated_at` anterior  
**When** el contenido ha cambiado o el estado es nuevo  
**Then** `updated_at` SHALL ser el instante de la consolidación

#### Scenario: Estado sin cambios conserva su fecha
**Given** un estado con `updated_at = "2026-09-01T10:00:00Z"` e idéntico contenido en dos consolidaciones  
**When** se consolidan  
**Then** el estado conserva `updated_at = "2026-09-01T10:00:00Z"`

#### Scenario: Estado nuevo o modificado sella el instante
**Given** un estado nuevo, o uno cuyo contenido ha cambiado  
**When** se consolida  
**Then** su `updated_at` es el instante de la consolidación

#### Scenario: La fecha devuelta por el LLM se ignora
**Given** un consolidado en el que el LLM propone una fecha artificial  
**When** se aplica la regla de la marca temporal  
**Then** el `updated_at` almacenado NO es el del LLM  
**And** es el instante de la consolidación

### Requirement: El estado SHALL respetar un presupuesto de tokens con un techo absoluto

El estado persistente SHALL caber en un presupuesto máximo de tokens, medido sobre su forma
inyectada —el JSON minificado— y leído de `settings.PERSISTENT_MEMORY_BUDGET_TOKENS` (por
defecto 500). Si el estado consolidado lo supera, el worker SHALL solicitar una **única**
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

### Requirement: El prompt del consolidador SHALL ser configurable y editable

El worker SHALL leer el prompt de consolidación de `settings.consolidator_prompt`, sembrado
por migración y editable sin recompilar. El prompt SHALL contener los marcadores
`{{ ESTADO_ACTUAL }}` y `{{ BLOQUE_DE_MENSAJES }}`, que SHALL sustituirse por el estado
persistente actual y por el bloque de mensajes. Si la clave falta, está vacía o no puede
leerse, el worker SHALL usar un prompt de respaldo mínimo y registrar un warning que
distinga la causa.

**Given** la tabla `settings` con `consolidator_prompt`  
**When** el worker prepara la consolidación  
**Then** SHALL leer el prompt de `settings.consolidator_prompt`  
**And** SHALL sustituir `{{ ESTADO_ACTUAL }}` por el JSON actual y
`{{ BLOQUE_DE_MENSAJES }}` por el bloque de mensajes

#### Scenario: Prompt sembrado en uso
**Given** una base migrada con `consolidator_prompt` sembrado  
**When** el worker consolida  
**Then** el mensaje de sistema contiene el texto sembrado  
**And** no quedan marcadores sin sustituir

#### Scenario: Respaldo si el prompt falta o está vacío
**Given** `consolidator_prompt` ausente o vacío  
**When** el worker consolida  
**Then** se usa un prompt de respaldo no vacío  
**And** se registra un warning que distingue si falta o si está vacío
