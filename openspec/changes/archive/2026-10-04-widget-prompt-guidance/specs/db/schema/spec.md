# Spec Delta: db/schema

## MODIFIED Requirements

### Requirement: Migración siembra los prompts del sistema en settings

**Given** una base de datos recién migrada
**When** se ejecuta `run_migrations()`
**Then** la tabla `settings` SHALL contener las claves `system_prompt`, `archivist_prompt` y `collapse_prompt`
**And** `system_prompt` SHALL contener el prompt de personalidad de Valet (con "asistente personal británico", "Modo Conciso (Predeterminado)", "Expandido" y "Emojis")
**And** `archivist_prompt` SHALL contener el prompt del archivista (con "archivista de memoria" y el placeholder `{{ BLOQUE_DE_MENSAJES }}`)
**And** `collapse_prompt` SHALL contener el prompt de resumen (con "Resume el siguiente texto")
**And** la migración de siembra SHALL NOT sobreescribir valores existentes no vacíos (personalizaciones del usuario)
**And** la migración de siembra SHALL rellenar valores ausentes o vacíos
**And** una migración aditiva posterior MAY anexar contenido nuevo al final del `system_prompt` sin eliminar el texto existente del usuario

#### Scenario: Base de datos nueva recibe los tres prompts
- **WHEN** se ejecuta `run_migrations()` sobre una base de datos vacía
- **THEN** `SELECT value FROM settings WHERE key='system_prompt'` devuelve un valor no vacío que contiene "asistente personal británico"
- **AND** `SELECT value FROM settings WHERE key='archivist_prompt'` devuelve un valor no vacío que contiene "{{ BLOQUE_DE_MENSAJES }}"
- **AND** `SELECT value FROM settings WHERE key='collapse_prompt'` devuelve un valor no vacío que contiene "Resume el siguiente texto"

#### Scenario: Valor vacío existente se rellena
- **GIVEN** una base de datos con `settings.system_prompt = ''`
- **WHEN** se ejecuta la migración de prompts
- **THEN** `settings.system_prompt` pasa a contener el prompt por defecto no vacío

#### Scenario: Personalización existente se respeta
- **GIVEN** una base de datos con `settings.system_prompt = 'Mi prompt personalizado'`
- **WHEN** se ejecuta la migración de prompts
- **THEN** `settings.system_prompt` sigue siendo `'Mi prompt personalizado'`

#### Scenario: Migración idempotente
- **WHEN** se ejecuta `run_migrations()` dos veces seguidas
- **THEN** no se produce error
- **AND** los tres prompts siguen presentes con un único valor por clave

## ADDED Requirements

### Requirement: La guía de uso de widgets SHALL anexarse al `system_prompt` por una migración aditiva e idempotente

Una migración SHALL anexar al final del valor de `settings.system_prompt` la sección
`# Instrucciones de Interfaz y Widgets Interactivos`, con estas reglas: un widget solo se muestra si se
**ejecuta** la llamada a `render_widget` (nunca se afirma haberlo mostrado sin invocarla); criterios de
activación (decisión entre 2 o más opciones, recolección de más de un dato, flujos paso a paso, datos
complejos o geográficos); restricción de texto simple (no invocar para explicaciones o datos directos); y
procesamiento de la respuesta del usuario sin volver a renderizar el widget salvo modificación explícita.
La migración SHALL anexar solo si la sección aún no está presente, SHALL preservar íntegro el texto
existente del usuario y SHALL NOT fallar ni crear la clave si `system_prompt` no existe o está vacío.

#### Scenario: Instalación nueva recibe la sección de widgets

**Given** una base de datos vacía
**When** se ejecuta `run_migrations()`
**Then** `settings.system_prompt` contiene la cabecera `# Instrucciones de Interfaz y Widgets Interactivos`
**And** menciona `render_widget`
**And** indica que un widget solo se muestra si se ejecuta la llamada a la herramienta
**And** enumera los criterios de activación (decisiones, recolección de datos, flujos paso a paso, datos complejos o geográficos)
**And** indica que para una explicación o un dato directo NO se invoca la herramienta

#### Scenario: Personalización existente se conserva y se le anexa la sección

**Given** una base de datos con `settings.system_prompt = 'Mi prompt personalizado'`
**When** se ejecuta la migración de guía de widgets
**Then** `settings.system_prompt` contiene `'Mi prompt personalizado'`
**And** también contiene la cabecera `# Instrucciones de Interfaz y Widgets Interactivos`
**And** el texto del usuario aparece antes que la sección

#### Scenario: Idempotente: la sección no se duplica

**When** se ejecuta `run_migrations()` dos veces seguidas
**Then** `settings.system_prompt` contiene una sola aparición de `# Instrucciones de Interfaz y Widgets Interactivos`
