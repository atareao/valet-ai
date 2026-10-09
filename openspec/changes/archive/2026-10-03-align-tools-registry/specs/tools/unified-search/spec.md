# Spec Delta: tools/unified-search

## Purpose

Búsqueda léxica transversal de Valet sobre el contenido indexado del perfil: en qué dimensiones
busca, cómo ordena los resultados por relevancia y cómo los presenta.

## ADDED Requirements

### Requirement: unified_search SHALL buscar en las dimensiones messages, notes, events y tasks

La herramienta SHALL consultar los índices FTS `messages_fts`, `notes_fts`, `events_fts` y
`tasks_fts`. Si no se especifica `dimensions`, SHALL consultar las cuatro; si se especifica una,
SHALL limitarse a ella.

#### Scenario: Sin dimensión consulta todas

**Given** contenido indexado en mensajes, notas, eventos y tareas
**When** se ejecuta `unified_search` solo con `query`
**Then** los resultados pueden proceder de las cuatro dimensiones
**And** cada resultado indica su dimensión de origen en `source` (`message`, `note`, `event` o `task`)

#### Scenario: Con dimensión se limita a una

**Given** contenido indexado en varias dimensiones
**When** se ejecuta `unified_search` con `dimensions: "notes"`
**Then** todos los resultados tienen `source = "note"`

### Requirement: unified_search SHALL ordenar los resultados por relevancia FTS

Cada resultado SHALL exponer su puntuación de relevancia en un campo numérico `rank`, y el conjunto
SHALL devolverse ordenado de forma ascendente por `rank` (menor valor = más relevante). El orden de
inserción de las tablas NO SHALL determinar el orden final.

#### Scenario: Los resultados llegan ordenados por rank

**Given** varias coincidencias con distinta relevancia
**When** se ejecuta `unified_search`
**Then** cada resultado incluye un campo numérico `rank`
**And** la secuencia de `rank` es ascendente

#### Scenario: El límite conserva las coincidencias más relevantes

**Given** más coincidencias que `limit`
**When** se ejecuta `unified_search` con `limit`
**Then** se devuelven como máximo `limit` resultados
**And** son los de menor `rank`

### Requirement: Cada resultado SHALL incluir dimensión de origen y extracto

Cada resultado SHALL incluir la dimensión de origen (`source`) y un extracto (`snippet`) con las
coincidencias resaltadas entre `<b>` y `</b>`.

#### Scenario: Extracto con coincidencias resaltadas

**Given** una coincidencia en cualquier dimensión
**When** se inspecciona su resultado
**Then** incluye `source` y `snippet`
**And** el término buscado aparece resaltado en el `snippet`
