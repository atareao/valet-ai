# skill-router-ui Specification

## Purpose
La cara visible del enrutador de skills en el diálogo de ajustes: encenderlo o apagarlo, elegir modelo y umbral, entender qué herramientas agrupa cada skill y editar los fragmentos de prompt que se activan con ellas.

## Requirements

### Requirement: La pestaña «Herramientas» SHALL exponer el control del enrutador

La pestaña «Herramientas» del diálogo de ajustes SHALL incluir un interruptor del enrutador, un campo numérico para el umbral **global** con rango de cero a uno, un campo para el modelo de decisiones, y la relación de skills con las herramientas que cubre cada una **y su umbral efectivo**. Para las skills que tengan un umbral propio distinto del global SHALL ofrecerse un campo para editarlo. Los valores SHALL persistirse a través del endpoint de settings existente y SHALL reflejar el valor vigente al abrir el diálogo.

#### Scenario: El control muestra el estado vigente
- **Given** el enrutador apagado con umbral global `0.1`
- **When** se abre la pestaña «Herramientas»
- **Then** el interruptor aparece apagado
- **And** el umbral global muestra `0.1`

#### Scenario: La relación de skills muestra el umbral efectivo de cada una
- **Given** un umbral global de `0.1` y un override de `0.2` para `widgets`
- **When** se inspecciona la relación de skills
- **Then** las skills de dominio figuran con umbral `0.1`
- **And** `widgets` figura con umbral `0.2`

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

La pestaña «Prompts» SHALL incluir una sub-pestaña que liste las skills **a partir del catálogo** —no por patrón de clave, para no mostrar entradas huérfanas— y ofrezca, para cada una, un área de texto editable con su pregunta, sus dos criterios y su fragmento de prompt, mostrando el valor vigente **efectivo** de `settings`. Cada campo sobrescrito SHALL aparecer marcado y SHALL ofrecer una acción de restaurar el valor por defecto, que SHALL vaciar la clave. Guardar SHALL NOT alterar las demás claves de settings ni el estado persistente. Si la consulta del catálogo falla, la vista SHALL degradar sin romper el formulario.

#### Scenario: Los fragmentos vigentes se muestran
- **Given** un catálogo con seis skills y un fragmento vigente para la skill de agenda
- **When** se abre la sub-pestaña de skills
- **Then** aparecen las seis, listadas desde el catálogo y no por patrón de clave
- **And** cada una muestra su pregunta, sus dos criterios y su fragmento
- **And** el área de texto de agenda muestra su fragmento vigente

#### Scenario: Un campo sobrescrito se marca y se puede restaurar
- **Given** una pregunta sobrescrita para una skill
- **When** se inspecciona su campo
- **Then** aparece marcado como sobrescrito
- **And** restaurar el valor por defecto vacía su clave en settings

#### Scenario: Guardar un fragmento no toca el resto
- **Given** un fragmento editado
- **When** se guarda
- **Then** se envía su clave `SKILL_<ID>_PROMPT` con el texto editado
- **And** el prompt del sistema, las preguntas, los criterios y las demás claves permanecen intactos

#### Scenario: Aviso al guardar un campo vacío
- **Given** un campo de criterio que el usuario ha dejado vacío
- **When** se guarda
- **Then** se advierte de que se usará el valor por defecto
- **And** el guardado no se bloquea

#### Scenario: La vista degrada si el catálogo falla
- **Given** una consulta del catálogo que falla
- **When** se abre la sub-pestaña de skills
- **Then** el formulario sigue renderizando
- **And** no se pierde ningún valor de settings

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
