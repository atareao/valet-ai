## MODIFIED Requirements

### Requirement: La guía de uso de widgets SHALL anexarse al `system_prompt` por una migración aditiva e idempotente

La migración `20261004000001_widget_prompt_guidance.sql` SHALL anexar al final del valor de
`settings.system_prompt` la sección `# Instrucciones de Interfaz y Widgets Interactivos`, con estas
reglas: un widget solo se muestra si se **ejecuta** la llamada a `render_widget` (nunca se afirma
haberlo mostrado sin invocarla); criterios de activación (decisión entre 2 o más opciones, recolección
de más de un dato, flujos paso a paso, datos complejos o geográficos); restricción de texto simple (no
invocar para explicaciones o datos directos); y procesamiento de la respuesta del usuario sin volver a
renderizar el widget salvo modificación explícita. Esa migración SHALL anexar solo si la sección aún no
está presente, SHALL preservar íntegro el texto existente del usuario y SHALL NOT fallar ni crear la
clave si `system_prompt` no existe o está vacío.

Sobre esa sección SHALL aplicarse después la migración `20261008000001_skill_router_tuning.sql`, que
SHALL retirarla del prompt base **solo si se encuentra verbatim** —dos saltos de línea iniciales
incluidos— y SHALL depositar su contenido en `settings.SKILL_WIDGETS_PROMPT`. Por tanto, el estado
final de una base de datos recién migrada SHALL ser: la sección **no** está en
`settings.system_prompt` y la guía **sí** está en `settings.SKILL_WIDGETS_PROMPT`. El contenido de la
guía y sus reglas vigentes SHALL ser los fijados por «La guía de widgets SHALL residir en el fragmento
de su skill» (`orchestrator/skill-router`), que es el requisito que gobierna su redacción.

#### Scenario: Instalación nueva recibe la sección de widgets

**Given** una base de datos vacía
**When** se ejecuta la migración `20261004000001_widget_prompt_guidance.sql`
**Then** `settings.system_prompt` contiene la cabecera `# Instrucciones de Interfaz y Widgets Interactivos`
**And** menciona `render_widget`
**And** indica que un widget solo se muestra si se ejecuta la llamada a la herramienta

#### Scenario: Personalización existente se conserva y se le anexa la sección

**Given** una base de datos con `settings.system_prompt = 'Mi prompt personalizado'`
**When** se ejecuta la migración `20261004000001_widget_prompt_guidance.sql`
**Then** `settings.system_prompt` contiene `'Mi prompt personalizado'`
**And** también contiene la cabecera `# Instrucciones de Interfaz y Widgets Interactivos`
**And** el texto del usuario aparece antes que la sección

#### Scenario: Idempotente: la sección no se duplica

**Given** una base de datos que ya contiene la sección de widgets en el prompt base
**When** se vuelve a ejecutar la migración `20261004000001_widget_prompt_guidance.sql`
**Then** `settings.system_prompt` contiene una sola aparición de `# Instrucciones de Interfaz y Widgets Interactivos`

#### Scenario: La sección termina en el fragmento, no en el prompt base

**Given** una base de datos recién migrada
**When** se leen `settings.system_prompt` y `settings.SKILL_WIDGETS_PROMPT`
**Then** el prompt base no contiene la cabecera `# Instrucciones de Interfaz y Widgets Interactivos`
**And** `settings.SKILL_WIDGETS_PROMPT` sí la contiene
**And** la guía es la fijada por «La guía de widgets SHALL residir en el fragmento de su skill»
