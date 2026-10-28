# tools/unified-search Specification

## Purpose
Búsqueda léxica transversal de Valet sobre el contenido indexado del perfil: en qué dimensiones
busca, cómo ordena los resultados por relevancia y cómo los presenta.

## Requirements

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

### Requirement: unified_search SHALL buscar en las dimensiones messages, notes y events

La herramienta SHALL consultar los índices FTS `messages_fts`, `notes_fts` y `events_fts`. Si no se
especifica `dimensions`, SHALL consultar las tres; si se especifica una, SHALL limitarse a ella. Un
valor de `dimensions` que no sea `messages`, `notes` o `events` SHALL tratarse como «sin filtro».

#### Scenario: Sin dimensión consulta las tres

- **Given** contenido indexado en mensajes, notas y eventos
- **When** se ejecuta `unified_search` solo con `query`
- **Then** los resultados pueden proceder de las tres dimensiones
- **And** cada resultado indica su origen en `source` (`message`, `note` o `event`)

#### Scenario: Con dimensión se limita a una

- **Given** contenido indexado en varias dimensiones
- **When** se ejecuta `unified_search` con `dimensions: "notes"`
- **Then** todos los resultados tienen `source = "note"`

#### Scenario: La dimensión `tasks` ya no existe

- **Given** una llamada con `dimensions: "tasks"`
- **When** se ejecuta `unified_search`
- **Then** no se consulta ninguna tabla FTS eliminada ni se produce un error de SQL
