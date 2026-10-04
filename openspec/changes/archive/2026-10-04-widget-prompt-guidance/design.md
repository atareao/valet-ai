# Design

## Context

- El `system_prompt` se siembra en `migrations/20260929000001_prompts.sql` (personalidad de Valet) y se
  lee de `settings.system_prompt` en cada construcción de la petición (`src/orchestrator/agent.rs`).
- La spec viva `db/schema` exige que la **migración de siembra** no sobreescriba valores no vacíos, con
  un escenario que congela «Personalización existente se respeta».
- El prompt es visible y editable desde la pestaña Prompt de Ajustes.
- La variante elegida por el usuario es **B2**: anexar por migración aditiva, no reemplazar.
- El texto de la sección lo fija el usuario (verbatim).

## Goals / Non-Goals

- **Goal:** que toda instalación (nueva o ya migrada) incorpore la guía de widgets al prompt, sin pisar
  personalizaciones, de forma idempotente.
- **Non-Goal:** componer la guía en código (ruta A), tocar la `description` de la tool, o cambiar el
  frontend. Los widgets de mapa/tabla/estadísticas que menciona el trigger «Datos complejos /
  Geográficos» se abordan en un change posterior.

## Decisions

- **Anexar, no sobreescribir.** La migración hace `UPDATE ... SET value = value || <sección>`
  condicionado a que el marcador no esté ya presente. Preserva el texto del usuario y, si ya añadió la
  sección a mano, no la duplica.
- **Marcador de idempotencia:** la cabecera de la sección,
  `# Instrucciones de Interfaz y Widgets Interactivos`. La condición es
  `instr(value, '# Instrucciones de Interfaz y Widgets Interactivos') = 0`.
- **Solo si el valor es no vacío.** Se anexa cuando `system_prompt` existe y no está vacío (lo garantiza
  la siembra previa, que corre antes por orden de fichero). Si no existiera, no se crea desde aquí.
- **Orden de migraciones.** El fichero se nombra `20261004000001_widget_prompt_guidance.sql`, posterior
  a la siembra (`20260929000001_prompts.sql`).
- **Texto del usuario, verbatim.** La sección se anexa tal cual la fijó el usuario; el anexado va al
  final del prompt (tras «Ejemplos de Comportamiento») para no reordenar lo existente.

## Contracts

### Sección anexada al `system_prompt` (verbatim)

```markdown
# Instrucciones de Interfaz y Widgets Interactivos

Dispones de la herramienta ejecutable `render_widget`.

## Reglas de Invocación
1. **Acción estricta:** Un widget SOLO se muestra en pantalla si ejecutas activamente la llamada a la función `render_widget`. NUNCA menciones que has mostrado, renderizado o dibujado un widget si no has ejecutado dicha herramienta en la misma respuesta.
2. **Criterios de Activación (Triggers):** DEBES invocar `render_widget` de forma explícita cuando la interacción cumpla cualquiera de estas condiciones:
   - **Toma de decisiones:** El usuario deba elegir entre 2 o más opciones.
   - **Recolección de datos:** Requieras más de un dato puntual (usa un formulario dinámico en lugar de repreguntar por texto).
   - **Flujos paso a paso:** Presentes un plan, guía o lista de tareas ejecutable.
   - **Datos complejos / Geográficos:** Muestres direcciones, rutas, mapas, tablas o estadísticas.
3. **Restricción de texto simple:** Si la respuesta se resuelve con una explicación conceptual o un dato directo, NO invoques la herramienta. Responde únicamente con texto plano.
4. **Procesamiento de respuestas:** Cuando el usuario interactúe con el widget, recibirás un mensaje de entrada con los datos seleccionados. Procesa la respuesta de inmediato y confirma el resultado sin volver a renderizar el widget, a menos que se requiera una modificación explícita.
```

### SQL de la migración (forma)

La sección se incrusta como literal multilínea (el texto no contiene comillas simples):

```sql
UPDATE settings
SET value = value || char(10) || char(10) || '<sección literal>',
    updated_at = datetime('now')
WHERE key = 'system_prompt'
  AND value IS NOT NULL
  AND value <> ''
  AND instr(value, '# Instrucciones de Interfaz y Widgets Interactivos') = 0;
```

## Risks / Trade-offs

- [El trigger «Datos complejos / Geográficos» nombra widgets que aún no existen] → El modelo puede
  intentar un `widget_name` no permitido; la tool lo rechaza y el turno continúa. El usuario decidió
  mantener el texto literal y planificar esos widgets (tabla/mapa/estadísticas) en un change posterior.
- [El prompt es editable: el usuario podría borrar la sección] → La migración corre una sola vez; si se
  borra, no vuelve. Aceptable: es su prompt.
- [El texto se anexa al final] → Es donde menos interfiere con la personalidad del prompt.

## Migration Plan

- Migración aditiva e idempotente; sin rollback destructivo. En una instalación ya migrada, el prompt
  crece con la sección una sola vez. `docker-compose.prod.yml` no se toca.
