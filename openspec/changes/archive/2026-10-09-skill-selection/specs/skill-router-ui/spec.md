## REMOVED Requirements

### Requirement: La pestaña «Herramientas» SHALL exponer el control del enrutador

### Requirement: La pestaña «Prompts» SHALL permitir editar los fragmentos por skill

## MODIFIED Requirements

### Requirement: La interfaz SHALL avisar sin bloquear ante configuraciones inertes

La interfaz SHALL advertir, sin impedir el guardado, cuando el enrutador está apagado (los fragmentos no se inyectan y las herramientas no se filtran, salvo las skills **deshabilitadas**, que nunca se exponen) y cuando el **umbral de una skill** está en los extremos cero o uno.

#### Scenario: Aviso con el enrutador apagado
- **Given** el enrutador apagado
- **When** se inspecciona la sección del enrutador
- **Then** se muestra un aviso de que el enrutado está inactivo
- **And** el guardado sigue permitido

#### Scenario: Aviso con umbral extremo
- **Given** el umbral de una skill en cero o en uno
- **When** se inspecciona el campo del umbral de esa skill
- **Then** se muestra un aviso de que ese valor desactiva el filtrado de esa skill o lo vuelve inalcanzable
- **And** el guardado sigue permitido

## ADDED Requirements

### Requirement: La pestaña «Enrutador de skills» SHALL exponer el control global del enrutador

La pestaña «Enrutador de skills» del diálogo de ajustes SHALL incluir un interruptor para **activar el enrutado** y un campo para el **modelo de decisiones**. SHALL NOT incluir un umbral global ni la lista de herramientas. Los valores SHALL persistirse a través del endpoint de settings existente y SHALL reflejar el valor vigente al abrir el diálogo.

#### Scenario: El control global muestra el estado vigente
- **Given** el enrutador apagado y un modelo configurado
- **When** se abre la pestaña «Enrutador de skills»
- **Then** el interruptor aparece apagado
- **And** el campo del modelo muestra el valor vigente

#### Scenario: No hay umbral global ni lista de herramientas
- **Given** la pestaña «Enrutador de skills» abierta
- **When** se inspecciona su contenido
- **Then** no figura ningún campo de umbral global
- **And** no figura la lista de herramientas con interruptores

#### Scenario: Activar y guardar persiste la configuración
- **Given** el enrutador apagado
- **When** se activa el interruptor y se guarda
- **Then** se envía `ROUTER_ENABLED` con el valor encendido
- **And** el resto de claves de settings se conservan

#### Scenario: El campo del modelo advierte de que es un modelo de decisiones
- **Given** la pestaña «Enrutador de skills» abierta
- **When** se inspecciona el campo del modelo
- **Then** su etiqueta o ayuda lo identifica como modelo de decisiones
- **And** permite el valor configurado por defecto

### Requirement: La pestaña «Skills» SHALL permitir configurar cada skill por separado

El diálogo de ajustes SHALL incluir una pestaña «Skills» de nivel superior —ya NO dentro de «Prompts»— que liste las skills **a partir del catálogo** (no por patrón de clave) y presente **una pestaña por skill**. Cada pestaña de skill SHALL ofrecer: un interruptor para **habilitarla o deshabilitarla**, un campo numérico para su **umbral** (rango cero a uno), y áreas de texto editables para su **pregunta**, su **criterio SÍ**, su **criterio NO** y su **fragmento de prompt**, mostrando el valor vigente **efectivo**. Cada campo sobrescrito SHALL aparecer marcado y ofrecer una acción de restaurar el valor por defecto, que SHALL vaciar la clave. Guardar SHALL persistir la habilitación, el umbral y los cuatro textos de la skill sin alterar las demás claves de settings. Si la consulta del catálogo falla, la vista SHALL degradar sin romper el formulario.

#### Scenario: Las skills se listan desde el catálogo, una pestaña por skill
- **Given** un catálogo con seis skills
- **When** se abre la pestaña «Skills»
- **Then** aparecen las seis, listadas desde el catálogo y no por patrón de clave
- **And** cada una tiene su propia pestaña

#### Scenario: Cada pestaña ofrece habilitación, umbral y los cuatro textos
- **Given** la pestaña de una skill
- **When** se inspecciona su contenido
- **Then** figura su interruptor de habilitación
- **And** figura su campo de umbral
- **And** figuran su pregunta, su criterio SÍ, su criterio NO y su fragmento

#### Scenario: Deshabilitar una skill se persiste
- **Given** una skill habilitada
- **When** se apaga su interruptor y se guarda
- **Then** se envía `ROUTER_SKILL_<ID>_ENABLED` con el valor apagado

#### Scenario: Un campo sobrescrito se marca y se puede restaurar
- **Given** una pregunta sobrescrita para una skill
- **When** se inspecciona su campo
- **Then** aparece marcado como sobrescrito
- **And** restaurar el valor por defecto vacía su clave en settings

#### Scenario: Aviso al guardar un campo vacío
- **Given** un campo de criterio que el usuario ha dejado vacío
- **When** se guarda
- **Then** se advierte de que se usará el valor por defecto
- **And** el guardado no se bloquea

#### Scenario: La vista degrada si el catálogo falla
- **Given** una consulta del catálogo que falla
- **When** se abre la pestaña «Skills»
- **Then** el formulario sigue renderizando
- **And** no se pierde ningún valor de settings
