-- Append the widget-usage guidance to the existing system_prompt.
-- Additive and idempotent: appends only when the section is not already present.
-- Never overwrites or removes the user's text.
UPDATE settings
SET value = value || char(10) || char(10) || '# Instrucciones de Interfaz y Widgets Interactivos

Dispones de la herramienta ejecutable `render_widget`.

## Reglas de Invocación
1. **Acción estricta:** Un widget SOLO se muestra en pantalla si ejecutas activamente la llamada a la función `render_widget`. NUNCA menciones que has mostrado, renderizado o dibujado un widget si no has ejecutado dicha herramienta en la misma respuesta.
2. **Criterios de Activación (Triggers):** DEBES invocar `render_widget` de forma explícita cuando la interacción cumpla cualquiera de estas condiciones:
   - **Toma de decisiones:** El usuario deba elegir entre 2 o más opciones.
   - **Recolección de datos:** Requieras más de un dato puntual (usa un formulario dinámico en lugar de repreguntar por texto).
   - **Flujos paso a paso:** Presentes un plan, guía o lista de tareas ejecutable.
   - **Datos complejos / Geográficos:** Muestres direcciones, rutas, mapas, tablas o estadísticas.
3. **Restricción de texto simple:** Si la respuesta se resuelve con una explicación conceptual o un dato directo, NO invoques la herramienta. Responde únicamente con texto plano.
4. **Procesamiento de respuestas:** Cuando el usuario interactúe con el widget, recibirás un mensaje de entrada con los datos seleccionados. Procesa la respuesta de inmediato y confirma el resultado sin volver a renderizar el widget, a menos que se requiera una modificación explícita.',
    updated_at = datetime('now')
WHERE key = 'system_prompt'
  AND value IS NOT NULL
  AND value <> ''
  AND instr(value, '# Instrucciones de Interfaz y Widgets Interactivos') = 0;