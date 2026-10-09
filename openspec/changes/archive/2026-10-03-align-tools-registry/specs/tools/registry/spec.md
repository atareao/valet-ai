# Spec Delta: tools/registry

## Purpose

El catálogo de herramientas que el orquestador de Valet ofrece al LLM: qué herramientas existen,
cuáles se exponen en cada petición y en qué condiciones se permite su ejecución.

## ADDED Requirements

### Requirement: El registry SHALL registrar las herramientas integradas notes y unified_search

La construcción del registry de producción SHALL registrar `notes` y `unified_search` además del
resto de herramientas integradas, garantizando nombres únicos.

#### Scenario: El registry de producción incluye las 12 herramientas

**Given** la aplicación Valet construida con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** `notes` y `unified_search` figuran entre ellas
**And** no hay dos herramientas con el mismo nombre

### Requirement: El registry SHALL omitir las herramientas deshabilitadas de las definiciones

`definitions()` SHALL devolver únicamente las herramientas cuyo estado sea habilitado.

#### Scenario: Una herramienta deshabilitada no se ofrece al LLM

**Given** un registry con las herramientas `weather` y `tasks`, y `weather` deshabilitada
**When** se solicitan las definiciones de herramientas
**Then** la lista contiene `tasks`
**And** la lista NO contiene `weather`

### Requirement: El registry SHALL rechazar la ejecución de herramientas deshabilitadas

`execute(nombre, args)` SHALL devolver un error para una herramienta deshabilitada, aunque esté
registrada, sin invocarla.

#### Scenario: La ejecución de una herramienta deshabilitada se rechaza

**Given** un registry con `weather` deshabilitada
**When** se invoca `execute("weather", args)`
**Then** se devuelve `Err(ToolError)`
**And** la herramienta NO se ejecuta

### Requirement: El estado habilitado SHALL proceder de la tabla tools y actualizarse en caliente

El registry SHALL cargar el estado habilitado desde la tabla `tools` al arrancar y SHALL reflejar de
inmediato las habilitaciones y deshabilitaciones disparadas desde la API de administración.

#### Scenario: Deshabilitar una herramienta surte efecto sin reiniciar

**Given** un registry con `weather` habilitada
**When** se deshabilita `weather` a través de la API de administración
**Then** las definiciones posteriores omiten `weather`
**And** `execute("weather", args)` devuelve error

#### Scenario: Habilitar de nuevo una herramienta la reexpone

**Given** un registry con `weather` deshabilitada
**When** se habilita `weather` a través de la API de administración
**Then** las definiciones posteriores vuelven a contener `weather`
**And** `execute("weather", args)` vuelve a invocarla
