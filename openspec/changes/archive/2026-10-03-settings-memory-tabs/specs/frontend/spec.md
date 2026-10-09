# Spec Delta: frontend

## MODIFIED Requirements

### Requirement: SettingsDialog SHALL display a Memoria tab with the four memory knobs

SettingsDialog SHALL display a "Memoria" tab that agrupa en un `Tabs` anidado dos sub-pestañas: «Episódica» y «Persistente». La sub-pestaña «Episódica» SHALL mostrar cuatro campos numéricos, uno por mando de memoria: `MEMORY_HALF_LIFE_DAYS`, `SIMILARITY_THRESHOLD`, `RAG_BUDGET_TOKENS` y `MEMORY_KNN_CANDIDATES`. La sub-pestaña «Persistente» SHALL mostrar el panel de memoria persistente descrito en la spec `persistent-memory-ui`. Los valores de la sub-pestaña «Episódica» SHALL cargarse de `GET /settings` y guardarse con `PUT /settings`, sin rutas nuevas de API. Al ser ajustables en caliente, un cambio guardado SHALL surtir efecto sin reiniciar.

**Given** el SettingsDialog está abierto en la pestaña "Memoria" con la sub-pestaña "Episódica" activa  
**When** se renderiza  
**Then** muestra cuatro campos numéricos: `MEMORY_HALF_LIFE_DAYS`, `SIMILARITY_THRESHOLD`, `RAG_BUDGET_TOKENS` y `MEMORY_KNN_CANDIDATES`  
**And** cada campo muestra el valor actual cargado de `GET /settings`  
**And** un botón "Guardar" persiste los cuatro valores vía `PUT /settings`

#### Scenario: Los cuatro campos están presentes
**Given** el SettingsDialog está abierto en la pestaña "Memoria" con la sub-pestaña "Episódica" activa  
**When** se renderiza  
**Then** existen los campos `MEMORY_HALF_LIFE_DAYS`, `SIMILARITY_THRESHOLD`, `RAG_BUDGET_TOKENS` y `MEMORY_KNN_CANDIDATES`

#### Scenario: Los valores se cargan desde la BD
**Given** `GET /settings` devuelve `MEMORY_HALF_LIFE_DAYS = 90` y `RAG_BUDGET_TOKENS = 800`  
**When** se abre la sub-pestaña "Episódica"  
**Then** el campo `MEMORY_HALF_LIFE_DAYS` muestra `90`  
**And** el campo `RAG_BUDGET_TOKENS` muestra `800`

#### Scenario: Los cuatro mandos se guardan
**Given** el usuario edita los cuatro campos  
**When** hace clic en "Guardar"  
**Then** `updateSettings` se llama con `{ MEMORY_HALF_LIFE_DAYS, SIMILARITY_THRESHOLD, RAG_BUDGET_TOKENS, MEMORY_KNN_CANDIDATES }`  
**And** se muestra el mensaje "Ajustes guardados"

#### Scenario: Editar un mando no borra los otros
**Given** el usuario modifica solo `SIMILARITY_THRESHOLD`  
**When** hace clic en "Guardar"  
**Then** los otros tres mandos se envían con sus valores actuales sin cambios

### Requirement: SettingsDialog SHALL fit all its top-level tabs without overflow

El `Modal` de SettingsDialog SHALL declarar un ancho de al menos **860 px** (se fija en 900) para que sus seis pestañas superiores —Perfil, Interfaz, Prompts, API Keys, Memoria y Generación— quepan en una sola fila, sin que ninguna se oculte en el desplegable de desbordamiento de antd.

**Given** el SettingsDialog abierto  
**When** se renderiza  
**Then** el ancho del diálogo es de al menos 860 px  
**And** sus seis pestañas superiores son alcanzables

#### Scenario: El diálogo es más ancho que el mínimo
**Given** el SettingsDialog abierto  
**When** se renderiza  
**Then** el ancho declarado del `Modal` es mayor o igual a 860 px

#### Scenario: La pestaña Memoria persistente es alcanzable
**Given** el SettingsDialog abierto  
**When** el usuario abre la pestaña "Memoria" y selecciona la sub-pestaña "Persistente"  
**Then** se muestra el panel de memoria persistente

#### Scenario: Existen exactamente seis pestañas superiores
**Given** el SettingsDialog abierto  
**When** el usuario busca las pestañas superiores  
**Then** existen exactamente seis: Perfil, Interfaz, Prompts, API Keys, Memoria y Generación  
**And** NO existe una pestaña superior "Memoria persistente"

## ADDED Requirements

### Requirement: SettingsDialog SHALL organize the memory panels in sub-tabs

Dentro de la pestaña «Memoria», SettingsDialog SHALL mostrar un `Tabs` anidado con dos sub-pestañas —«Episódica» y «Persistente»— siguiendo el patrón de las pestañas «Prompts» y «Generación». El `Tabs` anidado SHALL ir envuelto en una región etiquetada (`<section aria-label="Tipo de memoria">`), de modo que las tecnologías de asistencia la anuncien; antd no propaga `aria-label` al `role="tablist"`, así que un `aria-label` sobre el propio `Tabs` sería cosmético. Ninguna de las dos sub-pestañas SHALL usar `forceRender`: antd monta cada panel de forma perezosa, de modo que la sub-pestaña «Persistente» —y con ella el `GET` del estado persistente— solo se monta cuando se selecciona.

**Given** el SettingsDialog abierto en la pestaña "Memoria"  
**When** se renderiza  
**Then** existe un `Tabs` anidado con las sub-pestañas Episódica y Persistente  
**And** la sub-pestaña activa por defecto es "Episódica"

#### Scenario: Existen las dos sub-pestañas
**Given** el SettingsDialog abierto en la pestaña "Memoria"  
**When** se renderiza  
**Then** existen sub-pestañas con nombre Episódica y Persistente

#### Scenario: El Tabs anidado está dentro de una región «Tipo de memoria»
**Given** el SettingsDialog abierto en la pestaña "Memoria"  
**When** se renderiza  
**Then** existe una región accesible con nombre "Tipo de memoria" que contiene las sub-pestañas Episódica y Persistente

#### Scenario: Cambiar de sub-pestaña activa el panel correspondiente
**Given** el SettingsDialog abierto en "Memoria" con la sub-pestaña "Episódica" activa  
**When** el usuario selecciona la sub-pestaña "Persistente"  
**Then** el panel de memoria persistente queda visible  
**And** la sub-pestaña "Episódica" deja de estar visible

#### Scenario: El panel persistente no se monta hasta seleccionar su sub-pestaña
**Given** el SettingsDialog abierto en la pestaña "Memoria" con la sub-pestaña "Episódica" activa  
**When** se renderiza sin seleccionar "Persistente"  
**Then** el panel de memoria persistente NO está montado
