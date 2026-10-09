# persistent-memory-ui Specification

## ADDED Requirements

### Requirement: La lectura del estado persistente SHALL exponerse por HTTP

El sistema SHALL exponer `GET /api/persistent-memory`, que devuelve el estado de la Capa C y sus
cotas de tamaño en un objeto JSON con las claves `payload`, `updated_at`, `token_count`,
`budget_tokens`, `ceiling_tokens` e `is_empty`. `payload` SHALL ser el estado ya parseado (objeto
JSON) o `null` si no existe fila; `updated_at` SHALL ser la marca almacenada o `null`;
`token_count` SHALL medirse sobre la forma minificada con el mismo medidor que la Capa C;
`budget_tokens` SHALL leerse de `settings.PERSISTENT_MEMORY_BUDGET_TOKENS` (por defecto 500) y
`ceiling_tokens` SHALL ser su doble. La ausencia de fila SHALL leerse como estado vacío sin crear
la fila.

**Given** una base migrada  
**When** se invoca `GET /api/persistent-memory`  
**Then** SHALL devolverse 200 con el estado y las cotas  
**And** la lectura SHALL NOT crear la fila

#### Scenario: Estado existente se devuelve con sus cotas
**Given** un estado persistente de 120 tokens y un presupuesto de 500  
**When** se lee por HTTP  
**Then** `token_count` es 120, `budget_tokens` es 500 y `ceiling_tokens` es 1000  
**And** `payload` es el objeto almacenado y `updated_at` su marca

#### Scenario: Estado ausente se lee como vacío
**Given** una base migrada sin fila en `persistent_memory`  
**When** se lee por HTTP  
**Then** `payload` es `null`, `updated_at` es `null`, `is_empty` es `true` y `token_count` es 0  
**And** no se ha insertado ninguna fila

### Requirement: La escritura manual SHALL validar el esquema y respetar el techo

El sistema SHALL exponer `PUT /api/persistent-memory`, que recibe `payload` (objeto JSON) y SHALL
validarlo con el esquema versionado de la Capa C antes de escribir. Un `payload` con
`schema_version` ausente o distinta de 1, o con forma inválida, SHALL rechazarse sin escribir nada.
El estado aceptado SHALL medirse con el medidor de la Capa C: si supera el **techo absoluto**
(doble del presupuesto) SHALL rechazarse la escritura y conservarse el estado anterior; si supera
el presupuesto pero no el techo SHALL escribirse y devolverse un aviso; si cabe en el presupuesto
SHALL escribirse sin aviso. La escritura manual SHALL NOT invocar ninguna compresión con el LLM.
El `updated_at` SHALL resolverlo Rust con la regla de la Capa C (hash de contenido), ignorando
cualquier fecha del cliente.

**Given** un `payload` candidato y el presupuesto vigente  
**When** se escribe por HTTP  
**Then** SHALL validarse el esquema antes de medir  
**And** SHALL aplicarse el mismo control de presupuesto y techo que la consolidación  
**And** un rechazo SHALL dejar el estado anterior intacto

#### Scenario: Escritura válida dentro del presupuesto
**Given** un `payload` de versión 1 que cabe en el presupuesto  
**When** se escribe  
**Then** se devuelve 200 sin `warning`  
**And** el estado almacenado es el enviado

#### Scenario: Payload inválido no se escribe
**Given** un `payload` con `schema_version = 2`  
**When** se escribe  
**Then** se rechaza con un error de validación  
**And** el estado anterior permanece intacto

#### Scenario: Por encima del presupuesto se guarda con aviso
**Given** un `payload` válido que supera el presupuesto pero no el techo  
**When** se escribe  
**Then** se devuelve 200 con un `warning` no vacío  
**And** el estado se almacena

#### Scenario: Por encima del techo se rechaza
**Given** un `payload` válido que supera el doble del presupuesto  
**When** se escribe  
**Then** se rechaza con un error que indica el conteo, el presupuesto y el techo  
**And** el estado anterior permanece intacto

### Requirement: La escritura manual SHALL detectar cambios concurrentes

`PUT /api/persistent-memory` SHALL aceptar un campo opcional `expected_updated_at`. Cuando esté
presente y no coincida con el `updated_at` vigente —incluida la diferencia entre «sin fila» y «con
fila»— la escritura SHALL rechazarse con `409 Conflict` sin escribir nada. Cuando se omita, la
escritura SHALL proceder. Esto evita que la pantalla pise una consolidación del worker ocurrida
entre la carga y el guardado.

#### Scenario: La marca obsoleta provoca conflicto
**Given** un estado con `updated_at = "T1"` y una escritura con `expected_updated_at = "T0"`  
**When** se escribe  
**Then** se rechaza con 409  
**And** el estado vigente permanece intacto

#### Scenario: La marca vigente permite la escritura
**Given** un estado con `updated_at = "T1"` y una escritura con `expected_updated_at = "T1"`  
**When** se escribe  
**Then** se devuelve 200 y el estado se escribe

#### Scenario: Sin comprobación se procede
**Given** una escritura sin `expected_updated_at`  
**When** se escribe  
**Then** la escritura procede con independencia del `updated_at` vigente

### Requirement: El estado persistente SHALL poder vaciarse

El sistema SHALL exponer `DELETE /api/persistent-memory`, que elimina la fila `global_state`.
Tras el borrado, la lectura SHALL reportar estado vacío. La operación SHALL ser idempotente: si no
había estado, SHALL responder igualmente con éxito y sin error.

#### Scenario: Vaciar elimina la fila
**Given** un estado persistente existente  
**When** se invoca `DELETE /api/persistent-memory`  
**Then** la fila `global_state` deja de existir  
**And** la lectura posterior reporta estado vacío

#### Scenario: Vaciar un estado ausente es idempotente
**Given** una base sin fila en `persistent_memory`  
**When** se invoca `DELETE /api/persistent-memory`  
**Then** la respuesta es de éxito  
**And** no se crea ninguna fila

### Requirement: La interfaz SHALL mostrar y editar el estado

La interfaz SHALL ofrecer una pestaña «Memoria persistente» que cargue el estado al abrirse y
muestre su `updated_at`, su conteo de tokens frente al presupuesto —señalando cuando lo supera— y
un área de texto editable con el `payload` formateado. Antes de guardar SHALL validarse en cliente
que el texto sea JSON con `schema_version` 1 y solo claves de primer nivel permitidas; el servidor
SHALL seguir siendo la autoridad. El guardado SHALL enviar el `payload` y el `expected_updated_at`
cargado; un `409` SHALL recargar el estado y avisar del cambio concurrente, un error de validación
SHALL mostrarse sin escribir, y un `warning` de tamaño SHALL mostrarse como aviso. La pestaña
SHALL permitir además vaciar el estado con confirmación.

**Given** la pestaña abierta  
**When** se carga el estado  
**Then** SHALL mostrarse la marca temporal y los tokens frente al presupuesto  
**And** el área de texto SHALL contener el `payload` formateado

#### Scenario: Guardado correcto
**Given** un `payload` válido editado en la pestaña  
**When** se guarda  
**Then** SHALL enviarse con el `expected_updated_at` cargado  
**And** SHALL mostrarse confirmación

#### Scenario: Conflicto concurrente recarga
**Given** un guardado que recibe 409  
**When** se procesa la respuesta  
**Then** la pestaña SHALL recargar el estado vigente  
**And** SHALL avisar de que el estado cambió

#### Scenario: Exceso de tamaño se avisa
**Given** un guardado que devuelve un `warning` de tamaño  
**When** se procesa la respuesta  
**Then** SHALL mostrarse el aviso  
**And** el estado guardado SHALL reflejarse en la pestaña

### Requirement: El presupuesto SHALL poder verse y editarse desde la interfaz

La pestaña «Memoria persistente» SHALL mostrar el valor vigente de
`settings.PERSISTENT_MEMORY_BUDGET_TOKENS` y permitir editarlo y guardarlo. Un guardado del
presupuesto SHALL NOT alterar el estado persistente; solo la clave de configuración. El valor
editado SHALL reflejarse de inmediato en la comparación de tokens de la pestaña.

#### Scenario: Editar el presupuesto no toca el estado
**Given** un estado persistente existente y un presupuesto de 500  
**When** se cambia el presupuesto a 800 y se guarda  
**Then** `PERSISTENT_MEMORY_BUDGET_TOKENS` vale 800  
**And** el estado persistente permanece intacto  
**And** la comparación de tokens usa 800
