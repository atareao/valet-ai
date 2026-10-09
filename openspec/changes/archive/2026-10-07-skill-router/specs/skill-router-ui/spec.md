## Purpose

La cara visible del enrutador de skills en el diálogo de ajustes: encenderlo o apagarlo, elegir modelo y umbral, entender qué herramientas agrupa cada skill y editar los fragmentos de prompt que se activan con ellas.

## ADDED Requirements

### Requirement: La pestaña «Herramientas» SHALL exponer el control del enrutador

La pestaña «Herramientas» del diálogo de ajustes SHALL incluir un interruptor del enrutador, un campo numérico para el umbral con rango de cero a uno, un campo para el modelo de decisiones y la relación de skills con las herramientas que cubre cada una. Los valores SHALL persistirse a través del endpoint de settings existente y SHALL reflejar el valor vigente al abrir el diálogo.

#### Scenario: El control muestra el estado vigente
- **Given** el enrutador apagado con umbral `0.3`
- **When** se abre la pestaña «Herramientas»
- **Then** el interruptor aparece apagado
- **And** el umbral muestra `0.3`

#### Scenario: Encender y guardar persiste la configuración
- **Given** el enrutador apagado
- **When** se enciende el interruptor y se guarda
- **Then** se envía `ROUTER_ENABLED` con el valor encendido
- **And** el resto de claves de settings se conservan

#### Scenario: La relación de skills se explica en la interfaz
- **Given** la pestaña «Herramientas» abierta
- **When** se inspecciona la sección del enrutador
- **Then** figura cada skill enrutable con las herramientas que agrupa
- **And** se indica qué herramientas quedan fuera del enrutado por ser núcleo

#### Scenario: El campo del modelo advierte de que es un modelo de decisiones
- **Given** la pestaña «Herramientas» abierta
- **When** se inspecciona el campo del modelo
- **Then** su etiqueta o ayuda lo identifica como modelo de decisiones
- **And** permite el valor configurado por defecto

### Requirement: La pestaña «Prompts» SHALL permitir editar los fragmentos por skill

La pestaña «Prompts» SHALL incluir una sub-pestaña que liste las skills con un área de texto editable por fragmento, mostrando el valor vigente de `settings.SKILL_<ID>_PROMPT`. Guardar un fragmento SHALL NOT alterar las demás claves de settings ni el estado persistente.

#### Scenario: Los fragmentos vigentes se muestran
- **Given** un fragmento con texto para la skill de agenda
- **When** se abre la sub-pestaña de skills
- **Then** el área de texto de agenda muestra ese texto

#### Scenario: Guardar un fragmento no toca el resto
- **Given** un fragmento editado
- **When** se guarda
- **Then** se envía su clave `SKILL_<ID>_PROMPT` con el texto editado
- **And** el prompt del sistema y las demás claves permanecen intactas

### Requirement: La interfaz SHALL avisar sin bloquear ante configuraciones inertes

La interfaz SHALL advertir, sin impedir el guardado, cuando el enrutador está apagado (los fragmentos no se inyectan y las herramientas no se filtran) y cuando el umbral está en los extremos cero o uno.

#### Scenario: Aviso con el enrutador apagado
- **Given** el enrutador apagado
- **When** se inspecciona la sección del enrutador
- **Then** se muestra un aviso de que el enrutado está inactivo
- **And** el guardado sigue permitido

#### Scenario: Aviso con umbral extremo
- **Given** un umbral de cero o de uno
- **When** se inspecciona el campo del umbral
- **Then** se muestra un aviso de que ese valor desactiva el filtrado o lo vuelve inalcanzable
- **And** el guardado sigue permitido
