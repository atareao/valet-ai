# Spec Delta: persistent-memory-ui

## MODIFIED Requirements

### Requirement: La interfaz SHALL mostrar y editar el estado

La interfaz SHALL ofrecer una sub-pestaña «Persistente», dentro de la pestaña «Memoria» del diálogo de ajustes, que cargue el estado al abrirse y muestre su `updated_at`, su conteo de tokens frente al presupuesto —señalando cuando lo supera— y un área de texto editable con el `payload` formateado. Antes de guardar SHALL validarse en cliente que el texto sea JSON con `schema_version` 1 y solo claves de primer nivel permitidas; el servidor SHALL seguir siendo la autoridad. El guardado SHALL enviar el `payload` y el `expected_updated_at` cargado; un `409` SHALL recargar el estado y avisar del cambio concurrente, un error de validación SHALL mostrarse sin escribir, y un `warning` de tamaño SHALL mostrarse como aviso. La sub-pestaña SHALL permitir además vaciar el estado con confirmación.

**Given** la sub-pestaña «Persistente» abierta  
**When** se carga el estado  
**Then** SHALL mostrarse la marca temporal y los tokens frente al presupuesto  
**And** el área de texto SHALL contener el `payload` formateado

#### Scenario: Guardado correcto
**Given** un `payload` válido editado en la sub-pestaña  
**When** se guarda  
**Then** SHALL enviarse con el `expected_updated_at` cargado  
**And** SHALL mostrarse confirmación

#### Scenario: Conflicto concurrente recarga
**Given** un guardado que recibe 409  
**When** se procesa la respuesta  
**Then** la sub-pestaña SHALL recargar el estado vigente  
**And** SHALL avisar de que el estado cambió

#### Scenario: Exceso de tamaño se avisa
**Given** un guardado que devuelve un `warning` de tamaño  
**When** se procesa la respuesta  
**Then** SHALL mostrarse el aviso  
**And** el estado guardado SHALL reflejarse en la sub-pestaña

### Requirement: El presupuesto SHALL poder verse y editarse desde la interfaz

La sub-pestaña «Persistente» SHALL mostrar el valor vigente de `settings.PERSISTENT_MEMORY_BUDGET_TOKENS` y permitir editarlo y guardarlo. Un guardado del presupuesto SHALL NOT alterar el estado persistente; solo la clave de configuración. El valor editado SHALL reflejarse de inmediato en la comparación de tokens de la sub-pestaña.

#### Scenario: Editar el presupuesto no toca el estado
**Given** un estado persistente existente y un presupuesto de 500  
**When** se cambia el presupuesto a 800 y se guarda  
**Then** `PERSISTENT_MEMORY_BUDGET_TOKENS` vale 800  
**And** el estado persistente permanece intacto  
**And** la comparación de tokens usa 800
