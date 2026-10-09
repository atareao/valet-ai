# Spec Delta: orchestrator/agent

## ADDED Requirements

### Requirement: La petición SHALL abrir con un único mensaje de sistema

La petición al LLM SHALL abrir con **un solo** mensaje `role:"system"`, que reúna en este orden las secciones presentes: (1) el prompt de `settings.system_prompt`, (2) el hueco reservado para la memoria persistente —hoy vacío—, (3) la memoria episódica si la hay, y (4) la fecha, hora y ubicación si las hay. El mensaje SHALL preceder al historial de conversación. Las secciones presentes SHALL separarse con una línea en blanco, y ninguna sección ausente SHALL dejar título, marcador, separador ni línea en blanco.

**Given** un mensaje del usuario  
**When** el orquestador construye la petición al LLM  
**Then** la cabecera de la petición SHALL contener exactamente un mensaje `role:"system"` antes del historial  
**And** ese mensaje SHALL empezar por el contenido de `settings.system_prompt`  
**And** SHALL incluir después, y solo si existen, las secciones de memoria episódica, hueco persistente y fecha/hora/ubicación, en ese orden  
**And** ninguna sección ausente SHALL dejar título, marcador, separador ni línea en blanco

#### Scenario: Un solo mensaje de sistema
**Given** un orquestador con prompt, fichas inyectables y contexto de navegador  
**When** se construye la petición  
**Then** hay exactamente un mensaje `role:"system"` antes del historial  
**And** ese mensaje contiene el prompt, la sección episódica y la sección de fecha

#### Scenario: Orden de las secciones
**Given** un orquestador con prompt, fichas inyectables y contexto de navegador  
**When** se inspecciona el mensaje de sistema  
**Then** el prompt aparece antes que la sección episódica  
**And** la sección episódica aparece antes que la sección de fecha, hora y ubicación  
**And** la sección de fecha, hora y ubicación es la última

#### Scenario: Secciones ausentes sin rastro
**Given** un orquestador sin fichas inyectables y sin contexto de navegador  
**When** se construye la petición  
**Then** el mensaje de sistema contiene solo el prompt  
**And** no aparece ningún título, marcador ni línea en blanco de relleno

### Requirement: La sección de fecha, hora y ubicación SHALL cerrar el mensaje de sistema

Cuando el navegador aporte contexto (`BrowserContext`), la sección de fecha, hora y ubicación SHALL ser la **última** del mensaje de sistema, con el formato actual: `{fecha}.` y, si hay coordenadas, `Ubicación: {nombre} ({lat}, {lon}).` cuando se resuelva el nombre, o `Coordenadas: ({lat}, {lon}).` cuando no. SHALL omitirse por completo, sin dejar rastro, cuando no haya contexto de navegador.

**Given** una invocación con o sin `BrowserContext`  
**When** el orquestador construye la petición  
**Then** SHALL añadir la sección de fecha, hora y ubicación como última sección del mensaje de sistema si hay contexto  
**And** SHALL omitirla por completo si no lo hay

#### Scenario: La fecha, hora y ubicación cierran el mensaje
**Given** un `BrowserContext` con marca de tiempo y coordenadas  
**When** se construye la petición  
**Then** la última sección del mensaje de sistema es la de fecha, hora y ubicación  
**And** el texto contiene la marca de tiempo formateada y la ubicación o las coordenadas

#### Scenario: Sin contexto no hay sección
**Given** una invocación sin `BrowserContext`  
**When** se construye la petición  
**Then** el mensaje de sistema NO contiene ninguna sección de fecha, hora ni ubicación

### Requirement: El hueco de la memoria persistente SHALL quedar reservado y sin texto

Mientras no exista el proveedor de memoria persistente, la posición que le corresponde **entre el prompt y la memoria episódica** SHALL quedar reservada en el ensamblado del mensaje de sistema mediante un punto de inserción opcional. El ensamblado SHALL aceptar opcionalmente una sección de memoria persistente cuyo valor por defecto sea la ausencia total de texto: **SHALL NOT** inyectar título, marcador, comentario, separador ni línea en blanco mientras el hueco esté vacío.

**Given** un orquestador sin proveedor de memoria persistente  
**When** se construye la petición  
**Then** el mensaje de sistema NO SHALL contener ninguna sección de memoria persistente  
**And** NO SHALL aparecer ningún título, marcador ni comentario reservado para ella  
**And** el hueco SHALL existir en el ensamblado, **entre el prompt y la memoria episódica**, para que la feature siguiente lo rellene sin reordenar el mensaje

#### Scenario: Sin memoria persistente no hay rastro en el prompt
**Given** un orquestador sin memoria persistente  
**When** se compone el mensaje de sistema con prompt, fichas inyectables y contexto de navegador  
**Then** entre el prompt y la sección episódica no hay ningún texto  
**And** no aparece ningún marcador reservado de memoria persistente

## MODIFIED Requirements

### Requirement: Aislamiento estructural del bloque <episodic_memory>

El bloque de memoria episódica SHALL ir delimitado por las etiquetas `<episodic_memory>` dentro de la sección `# CONTEXTO DE MEMORIA EPISÓDICA (CAPA B)`, con la instrucción explícita de que son antecedentes y no parte de la conversación actual, y SHALL NOT usar el prefijo `[Memory context]`. El bloque SHALL ser una sección del único mensaje de sistema, que va **antes** del historial de conversación.

**Given** un orquestador con fichas que superan el umbral  
**When** se construye la petición  
**Then** el mensaje de sistema contiene `# CONTEXTO DE MEMORIA EPISÓDICA (CAPA B)` y las etiquetas `<episodic_memory>`  
**And** contiene la instrucción de que son antecedentes y no parte del turno actual  
**And** NO contiene el prefijo `[Memory context]`  
**And** es una sección del único mensaje de sistema, que aparece antes de los mensajes del historial de conversación

#### Scenario: Etiquetas e instrucción presentes
**Given** fichas inyectables  
**When** se inspecciona la sección de memoria del mensaje de sistema  
**Then** contiene `<episodic_memory>` y su cierre  
**And** contiene la instrucción de no confundir los antecedentes con el turno actual

#### Scenario: El bloque precede al historial
**Given** un historial de conversación cargado por `list_by_token_budget`  
**When** se ordenan los mensajes de la petición  
**Then** el mensaje de sistema que contiene `<episodic_memory>` aparece antes del primer mensaje del historial

#### Scenario: No queda el prefijo antiguo
**Given** fichas inyectables  
**When** se construye el prompt  
**Then** NO aparece el literal `[Memory context]` en ningún mensaje

### Requirement: Inyección automática del bloque de memoria episódica en la ruta de streaming

**Given** un mensaje del usuario  
**When** el orquestador construye la petición al LLM  
**Then** SHALL recuperar memorias episódicas por similitud semántica en **cada** mensaje, con independencia de la estrategia de contexto  
**And** SHALL componer el bloque de memoria en código, no desde un placeholder de `settings.system_prompt`  
**And** SHALL incluir el bloque como una sección dentro del único mensaje de sistema, nunca como un mensaje `role:"system"` independiente  
**And** SHALL omitir la sección por completo cuando no haya ninguna ficha que supere el umbral, sin texto de relleno del tipo "no hay antecedentes"

#### Scenario: Se inyecta memoria cuando hay fichas sobre el umbral
**Given** un orquestador con pool, provider y al menos una ficha que supera `SIMILARITY_THRESHOLD`  
**When** se construye la petición  
**Then** el mensaje de sistema contiene la sección de memoria episódica  
**And** la sección incluye el contenido de esa ficha

#### Scenario: El bloque se omite por completo sin fichas
**Given** un orquestador con pool y provider pero sin fichas que superen el umbral  
**When** se construye la petición  
**Then** el mensaje de sistema NO contiene la sección de memoria episódica ni las etiquetas `<episodic_memory>`  
**And** NO aparece ningún texto de relleno del tipo "no hay antecedentes"

#### Scenario: El bloque no depende de un placeholder editable
**Given** un `settings.system_prompt` sin ningún placeholder de memoria  
**When** se construye la petición  
**Then** el bloque de memoria se compone igualmente en código  
**And** borrar o editar `system_prompt` NO desactiva la memoria
