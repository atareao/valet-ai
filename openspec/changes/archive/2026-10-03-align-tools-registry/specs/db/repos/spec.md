# Spec Delta: db/repos

## ADDED Requirements

### Requirement: ToolsRepo SHALL reconciliar la tabla tools con el registry

La tabla `tools` SHALL reconciliarse con el catálogo de herramientas registradas: SHALL insertar las
herramientas registradas ausentes en la tabla con la descripción del registry y `enabled = 1`, SHALL
eliminar las filas cuyo nombre ya no corresponde a ninguna herramienta registrada, y SHALL preservar
el valor de `enabled` de las filas existentes actualizando solo su descripción. La operación SHALL ser
idempotente.

#### Scenario: Alta de herramientas nuevas

**Given** una tabla `tools` con `calendar` y `weather`
**When** se reconcilia con un registry que además contiene `notes` y `unified_search`
**Then** `notes` y `unified_search` quedan insertadas con `enabled = 1`

#### Scenario: Baja de herramientas obsoletas

**Given** una tabla `tools` que contiene la fila `geo`, que no está en el registry
**When** se reconcilia con el registry
**Then** la fila `geo` se elimina

#### Scenario: Se preserva el estado habilitado

**Given** una tabla `tools` con `weather` y `enabled = 0`
**When** se reconcilia con el registry
**Then** `weather` sigue con `enabled = 0`
**And** su descripción se actualiza con la del registry

#### Scenario: La reconciliación es idempotente

**Given** una tabla `tools` ya reconciliada con el registry
**When** se reconcilia de nuevo
**Then** el contenido de la tabla no cambia
