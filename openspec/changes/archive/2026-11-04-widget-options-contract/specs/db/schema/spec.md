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
guía y sus reglas vigentes SHALL ser los fijados por «El fragmento de la skill widgets SHALL ser un
recordatorio breve que remita a render_widget» (`orchestrator/skill-router`), que es el requisito que
gobierna su redacción.

La migración `20261104000001_widget_options_contract.sql` SHALL sustituir `settings.SKILL_WIDGETS_PROMPT` por el
texto que declara la forma que el servidor sabe leer (ver `orchestrator/skill-router`) **solo si el valor
actual es byte a byte uno de los dos textos conocidos** —el recordatorio sembrado por `20261102000001` o la
guía imperativa de 3.812 caracteres— y SHALL NOT pisar ningún otro valor. La migración SHALL ser idempotente:
reejecutarla no SHALL cambiar nada.

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
**And** `settings.SKILL_WIDGETS_PROMPT` contiene el recordatorio breve (no la guía extensa) y por tanto ya no lleva esa cabecera
**And** el recordatorio es el fijado por «El fragmento de la skill widgets SHALL ser un recordatorio breve que remita a render_widget»

#### Scenario: La guía reconocida se sustituye por el contrato de forma
**Given** una base de datos cuyo `SKILL_WIDGETS_PROMPT` es el recordatorio sembrado por `20261102000001`
**When** se ejecuta la migración `20261104000001_widget_options_contract.sql`
**Then** `SKILL_WIDGETS_PROMPT` pasa a pedir la tabla con cabecera de opciones
**And** reejecutar la migración no cambia nada

#### Scenario: Un valor del usuario no se toca
**Given** una base de datos cuyo `SKILL_WIDGETS_PROMPT` ha sido editado a un texto distinto de los dos conocidos
**When** se ejecuta la migración `20261104000001_widget_options_contract.sql`
**Then** el valor del usuario queda intacto
