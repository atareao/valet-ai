# Spec Delta: orchestrator/agent

## MODIFIED Requirements

### Requirement: La petición SHALL abrir con un único mensaje de sistema

La petición al LLM SHALL abrir con **un solo** mensaje `role:"system"`, que reúna en este
orden las secciones presentes: (1) el prompt de `settings.system_prompt`, (2) la sección con el
nombre del usuario, (3) la sección de memoria persistente si la hay, (4) la memoria episódica si
la hay, y (5) la fecha, hora y ubicación si las hay. El mensaje SHALL preceder al historial de
conversación. Las secciones presentes SHALL separarse con una línea en blanco, y ninguna sección
ausente SHALL dejar título, marcador, separador ni línea en blanco.

**Given** un mensaje del usuario  
**When** el orquestador construye la petición al LLM  
**Then** la cabecera de la petición SHALL contener exactamente un mensaje `role:"system"` antes del historial  
**And** ese mensaje SHALL empezar por el contenido de `settings.system_prompt`  
**And** SHALL incluir después, y solo si existen, las secciones de nombre de usuario, memoria persistente, memoria episódica y fecha/hora/ubicación, en ese orden  
**And** ninguna sección ausente SHALL dejar título, marcador, separador ni línea en blanco

#### Scenario: Un solo mensaje de sistema
**Given** un orquestador con prompt, fichas inyectables y contexto de navegador  
**When** se construye la petición  
**Then** hay exactamente un mensaje `role:"system"` antes del historial  
**And** ese mensaje contiene el prompt, la sección episódica y la sección de fecha

#### Scenario: Orden de las secciones
**Given** un orquestador con prompt, nombre de usuario, memoria persistente no vacía, fichas inyectables y contexto de navegador  
**When** se inspecciona el mensaje de sistema  
**Then** el prompt aparece antes que la sección con el nombre del usuario  
**And** la sección con el nombre del usuario aparece antes que la sección de memoria persistente  
**And** la sección de memoria persistente aparece antes que la sección episódica  
**And** la sección episódica aparece antes que la sección de fecha, hora y ubicación  
**And** la sección de fecha, hora y ubicación es la última

#### Scenario: Secciones ausentes sin rastro
**Given** un orquestador sin nombre de usuario, sin fichas inyectables y sin contexto de navegador  
**When** se construye la petición  
**Then** el mensaje de sistema contiene solo el prompt  
**And** no aparece ningún título, marcador ni línea en blanco de relleno

## ADDED Requirements

### Requirement: El nombre del usuario SHALL inyectarse en el mensaje de sistema

El orquestador SHALL leer el nombre del usuario desde `profiles.name` —por el `profile_id` recibido—
en **cada** construcción de la petición, y SHALL inyectarlo como una sección compuesta en código
—nunca desde un placeholder de `settings.system_prompt`— situada **entre el prompt y la memoria
persistente**. La sección SHALL usar el título `# USUARIO` y el texto `El nombre del usuario es {name}. Dirígete a él por su nombre cuando sea natural, sin repetirlo en cada respuesta.`.
SHALL omitirse por completo, sin dejar rastro, cuando el nombre esté vacío tras recortar espacios o
sea el valor por defecto `Valet User`. Si la lectura del perfil falla, SHALL loguear un warning y
omitir la sección sin abortar la petición.

**Given** un mensaje del usuario y un perfil con nombre no vacío y distinto del valor por defecto  
**When** el orquestador construye la petición  
**Then** el mensaje de sistema SHALL contener una sección `# USUARIO` con `El nombre del usuario es {name}.` y la guía de uso del nombre  
**And** esa sección SHALL situarse entre el prompt y la sección de memoria persistente

#### Scenario: El nombre real se inyecta
**Given** un perfil con `name = "Lorenzo"`  
**When** se construye la petición  
**Then** el mensaje de sistema contiene `El nombre del usuario es Lorenzo. Dirígete a él por su nombre cuando sea natural, sin repetirlo en cada respuesta.`  
**And** la sección aparece después del prompt

#### Scenario: Nombre vacío no deja rastro
**Given** un perfil con `name` vacío o solo espacios  
**When** se construye la petición  
**Then** el mensaje de sistema NO contiene el título `# USUARIO` ni la línea con el nombre  
**And** no queda ningún separador de relleno

#### Scenario: El nombre por defecto no se inyecta
**Given** un perfil recién creado con `name = "Valet User"`  
**When** se construye la petición  
**Then** el mensaje de sistema NO contiene la sección `# USUARIO`

#### Scenario: El nombre no depende de un placeholder editable
**Given** un `settings.system_prompt` sin ningún placeholder de nombre  
**When** se construye la petición  
**Then** la sección del nombre se compone igualmente en código
