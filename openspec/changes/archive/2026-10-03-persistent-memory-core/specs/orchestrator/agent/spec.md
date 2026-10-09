# Spec Delta: orchestrator/agent

## MODIFIED Requirements

### Requirement: La petición SHALL abrir con un único mensaje de sistema

La petición al LLM SHALL abrir con **un solo** mensaje `role:"system"`, que reúna en este
orden las secciones presentes: (1) el prompt de `settings.system_prompt`, (2) la sección de
memoria persistente si la hay, (3) la memoria episódica si la hay, y (4) la fecha, hora y
ubicación si las hay. El mensaje SHALL preceder al historial de conversación. Las secciones
presentes SHALL separarse con una línea en blanco, y ninguna sección ausente SHALL dejar
título, marcador, separador ni línea en blanco.

**Given** un mensaje del usuario  
**When** el orquestador construye la petición al LLM  
**Then** la cabecera de la petición SHALL contener exactamente un mensaje `role:"system"` antes del historial  
**And** ese mensaje SHALL empezar por el contenido de `settings.system_prompt`  
**And** SHALL incluir después, y solo si existen, las secciones de memoria persistente, memoria episódica y fecha/hora/ubicación, en ese orden  
**And** ninguna sección ausente SHALL dejar título, marcador, separador ni línea en blanco

#### Scenario: Un solo mensaje de sistema
**Given** un orquestador con prompt, fichas inyectables y contexto de navegador  
**When** se construye la petición  
**Then** hay exactamente un mensaje `role:"system"` antes del historial  
**And** ese mensaje contiene el prompt, la sección episódica y la sección de fecha

#### Scenario: Orden de las secciones
**Given** un orquestador con prompt, memoria persistente no vacía, fichas inyectables y contexto de navegador  
**When** se inspecciona el mensaje de sistema  
**Then** el prompt aparece antes que la sección de memoria persistente  
**And** la sección de memoria persistente aparece antes que la sección episódica  
**And** la sección episódica aparece antes que la sección de fecha, hora y ubicación  
**And** la sección de fecha, hora y ubicación es la última

#### Scenario: Secciones ausentes sin rastro
**Given** un orquestador sin fichas inyectables y sin contexto de navegador  
**When** se construye la petición  
**Then** el mensaje de sistema contiene solo el prompt  
**And** no aparece ningún título, marcador ni línea en blanco de relleno

## REMOVED Requirements

### Requirement: El hueco de la memoria persistente SHALL quedar reservado y sin texto

**Reason**: el hueco reservado se rellena con la sección de memoria persistente real (Capa C).  
**Migration**: no requiere acción; la sección se inyecta automáticamente entre el prompt y la
memoria episódica cuando el estado no está vacío, y se omite por completo cuando lo está.

## ADDED Requirements

### Requirement: La sección de memoria persistente SHALL inyectarse entre el prompt y la episódica

Cuando exista un estado persistente no vacío, el orquestador SHALL inyectar su sección
**entre el prompt y la memoria episódica**, como el `payload` en **JSON minificado** precedido
de un encabezado corto. El estado SHALL leerse en **cada** construcción de la petición.
SHALL omitirse por completo, sin dejar rastro —ni encabezado, ni marcador, ni línea en
blanco—, cuando el estado esté vacío o no exista.

**Given** un estado persistente no vacío  
**When** el orquestador construye la petición  
**Then** el mensaje de sistema SHALL contener la sección de memoria persistente con el JSON minificado  
**And** esa sección SHALL situarse entre el prompt y la sección episódica

**Given** un estado persistente vacío o inexistente  
**When** el orquestador construye la petición  
**Then** el mensaje de sistema NO SHALL contener ninguna sección de memoria persistente  
**And** NO SHALL aparecer encabezado, marcador ni línea en blanco de relleno

#### Scenario: Estado no vacío se inyecta entre el prompt y la episódica
**Given** un orquestador con un estado persistente no vacío, fichas inyectables y contexto de navegador  
**When** se construye la petición  
**Then** el mensaje de sistema contiene el JSON minificado del estado  
**And** la sección persistente está entre el prompt y la sección episódica

#### Scenario: Estado vacío no deja rastro
**Given** un orquestador sin estado persistente y con prompt, fichas inyectables y contexto de navegador  
**When** se construye la petición  
**Then** entre el prompt y la sección episódica no hay texto de memoria persistente  
**And** no aparece ningún encabezado ni marcador reservado
