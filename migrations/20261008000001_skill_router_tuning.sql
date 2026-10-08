-- Skill-router tuning (skill-router-tuning D6/D7) + widget-guide relocation (D5).
--
-- Same idiom as `20261007000001_skill_router.sql` and
-- `20260929000001_prompts.sql`: idempotent, and it never overwrites a value the
-- user has customised. The router knobs are read on every turn, so editing them
-- takes effect without a restart.
--
-- Why each block:
--
--   1) ROUTER_THRESHOLD 0.3 -> 0.10 (measured). The harness measured the closed
--      six-domain catalog at a global threshold of 0.10. The 0.3 default came
--      from the old eight-skill catalog and no longer matches the measured
--      configuration (design.md D6). Only the exact seeded value is bumped: a
--      user value is left untouched.
--
--   2) ROUTER_HISTORY_TURNS 2 -> 6 (measured). The measurement was run with six
--      turns of history; the dominant failure class was continuations ("ahora
--      sí", "y el planito?") whose need is carried by the conversation, not by
--      the isolated message. Production was clipping to two (design.md D7).
--      Again, only the exact seeded value is bumped.
--
--   3) ROUTER_THRESHOLD_WIDGETS = 0.20. The widget is discretionary and its
--      base probability (p50 0.21) sits above any low threshold, so it needs its
--      own, higher threshold instead of riding on the global one (design.md D6).
--
--   4) Seed the prompt fragments of the five new/regrouped skills. The
--      `pendientes`, `recuerdos`, `entorno` and `web` fragments fuse the old
--      fine-grained ones; `SKILL_AGENDA_PROMPT` is untouched. The old keys
--      (`SKILL_TAREAS_PROMPT`, ...) are left as inert rows: user content is
--      never deleted, and the UI now lists fragments from the catalog.
--
--   5) Move the widget-usage guide out of the base prompt. `render_widget` is no
--      longer always exposed (it is routed with the `widgets` skill), so a
--      permanent instruction to use it would be unfulfillable when the skill is
--      inactive. The block is copied verbatim into `SKILL_WIDGETS_PROMPT` and
--      removed from `settings.system_prompt` only if it is found verbatim; if the
--      user edited it, nothing is removed and the fragment is seeded anyway. The
--      fragment assembler's anti-duplication rule stays in place as a safety net
--      (design.md D5).

-- 1) Measured global threshold: 0.3 -> 0.10, only when still at our default.
UPDATE settings SET value = '0.10', updated_at = datetime('now')
WHERE key = 'ROUTER_THRESHOLD' AND value = '0.3';

-- 2) Measured history window: 2 -> 6, only when still at our default.
UPDATE settings SET value = '6', updated_at = datetime('now')
WHERE key = 'ROUTER_HISTORY_TURNS' AND value = '2';

-- 3) The widget's own, higher threshold (per-skill override).
INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_THRESHOLD_WIDGETS', '0.20', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 4) Fragments of the regrouped domains (see header). Only seeded when missing.
INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_PENDIENTES_PROMPT', '# SKILL ACTIVA: PENDIENTES
- Distingue la forma: algo por hacer es una tarea (`tasks`), un aviso puntual a una hora es un recordatorio (`reminders`) y una cita con aviso previo es un evento (`calendar`, con `reminder_minutes_before`).
- Las tareas nacen en `inbox`; `complete_task` las pasa a `done` y `update_task` mueve su estado GTD. `due_date` es una fecha límite informativa: no dispara ningún aviso por sí sola.
- `update_task`, `complete_task`, `delete_task`, `dismiss_reminder` y `snooze_reminder` exigen el `id`: obtenlo con `list_tasks` o `list_reminders` en el mismo turno y no lo inventes.
- `set_reminder` exige fecha y hora absolutas: convierte «en media hora» o «mañana» a ISO 8601 antes de llamar. Para un aviso que se repite, usa un evento con `rrule`, no un recordatorio.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_RECUERDOS_PROMPT', '# SKILL ACTIVA: RECUERDOS
- Empieza por `unified_search` cuando no se sepa en qué dimensión vive el dato o se pregunte por algo ya hablado; si el dominio está claro, acótalo con `dimensions` en lugar de recorrer todas las dimensiones.
- `unified_search` busca también en los mensajes: es la vía para recordar conversaciones anteriores, no solo datos guardados.
- Al crear una nota elige la categoría con criterio: `fact` para un dato objetivo, `idea` para una ocurrencia y `journal` para una reflexión; las `tags` van separadas por comas para poder recuperarla por tema.
- `delete_note` exige el `id`: localízalo con `list_notes` antes de borrar y no lo inventes.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_ENTORNO_PROMPT', '# SKILL ACTIVA: ENTORNO
- Secuencia: para el tiempo, si el usuario nombra un lugar resuelve sus coordenadas con `geocode` y consulta `weather` en el mismo turno; si no lo nombra, parte de su ubicación guardada con `get_current_location`. No le pidas a él la latitud y la longitud.
- El sentido de las dos geocodificaciones no se confunde: `geocode` convierte una dirección o un lugar en coordenadas y `reverse_geocode` convierte coordenadas en una dirección.
- Para buscar cerca del usuario, pásale a `search_places` las coordenadas obtenidas antes (`get_current_location` o `geocode`) y un `radius`; sin coordenadas la búsqueda pierde el entorno.
- Para una previsión pasa `date` en formato YYYY-MM-DD (convierte «mañana» a fecha absoluta); sin `date`, `weather` devuelve el tiempo actual.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_WEB_PROMPT', '# SKILL ACTIVA: WEB
- Resérvala para información externa o cambiante (noticias, precios, documentación); lo que vive en los datos del usuario se busca con `unified_search` o con la herramienta propia de su dominio.
- Reformula la petición como palabras clave, no como la frase completa del usuario.
- Apoya la respuesta en la URL de los resultados y no completes con conocimiento previo lo que la búsqueda no confirme.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 5a) Seed the widget guide verbatim (without the two leading newlines the
--     original migration prefixed) as the widgets fragment.
INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_WIDGETS_PROMPT', '# Instrucciones de Interfaz y Widgets Interactivos

Dispones de la herramienta ejecutable `render_widget`.

## Reglas de Invocación
1. **Acción estricta:** Un widget SOLO se muestra en pantalla si ejecutas activamente la llamada a la función `render_widget`. NUNCA menciones que has mostrado, renderizado o dibujado un widget si no has ejecutado dicha herramienta en la misma respuesta.
2. **Criterios de Activación (Triggers):** DEBES invocar `render_widget` de forma explícita cuando la interacción cumpla cualquiera de estas condiciones:
   - **Toma de decisiones:** El usuario deba elegir entre 2 o más opciones.
   - **Recolección de datos:** Requieras más de un dato puntual (usa un formulario dinámico en lugar de repreguntar por texto).
   - **Flujos paso a paso:** Presentes un plan, guía o lista de tareas ejecutable.
   - **Datos complejos / Geográficos:** Muestres direcciones, rutas, mapas, tablas o estadísticas.
3. **Restricción de texto simple:** Si la respuesta se resuelve con una explicación conceptual o un dato directo, NO invoques la herramienta. Responde únicamente con texto plano.
4. **Procesamiento de respuestas:** Cuando el usuario interactúe con el widget, recibirás un mensaje de entrada con los datos seleccionados. Procesa la respuesta de inmediato y confirma el resultado sin volver a renderizar el widget, a menos que se requiera una modificación explícita.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 5b) Remove the widget guide from the base prompt, only when it is byte-for-byte
--     the block we appended (two leading newlines included). `instr`/`replace`
--     match the exact string, so an edited block is never touched.
UPDATE settings SET value = replace(value, char(10) || char(10) || '# Instrucciones de Interfaz y Widgets Interactivos

Dispones de la herramienta ejecutable `render_widget`.

## Reglas de Invocación
1. **Acción estricta:** Un widget SOLO se muestra en pantalla si ejecutas activamente la llamada a la función `render_widget`. NUNCA menciones que has mostrado, renderizado o dibujado un widget si no has ejecutado dicha herramienta en la misma respuesta.
2. **Criterios de Activación (Triggers):** DEBES invocar `render_widget` de forma explícita cuando la interacción cumpla cualquiera de estas condiciones:
   - **Toma de decisiones:** El usuario deba elegir entre 2 o más opciones.
   - **Recolección de datos:** Requieras más de un dato puntual (usa un formulario dinámico en lugar de repreguntar por texto).
   - **Flujos paso a paso:** Presentes un plan, guía o lista de tareas ejecutable.
   - **Datos complejos / Geográficos:** Muestres direcciones, rutas, mapas, tablas o estadísticas.
3. **Restricción de texto simple:** Si la respuesta se resuelve con una explicación conceptual o un dato directo, NO invoques la herramienta. Responde únicamente con texto plano.
4. **Procesamiento de respuestas:** Cuando el usuario interactúe con el widget, recibirás un mensaje de entrada con los datos seleccionados. Procesa la respuesta de inmediato y confirma el resultado sin volver a renderizar el widget, a menos que se requiera una modificación explícita.', ''), updated_at = datetime('now')
WHERE key = 'system_prompt'
  AND instr(value, char(10) || char(10) || '# Instrucciones de Interfaz y Widgets Interactivos

Dispones de la herramienta ejecutable `render_widget`.

## Reglas de Invocación
1. **Acción estricta:** Un widget SOLO se muestra en pantalla si ejecutas activamente la llamada a la función `render_widget`. NUNCA menciones que has mostrado, renderizado o dibujado un widget si no has ejecutado dicha herramienta en la misma respuesta.
2. **Criterios de Activación (Triggers):** DEBES invocar `render_widget` de forma explícita cuando la interacción cumpla cualquiera de estas condiciones:
   - **Toma de decisiones:** El usuario deba elegir entre 2 o más opciones.
   - **Recolección de datos:** Requieras más de un dato puntual (usa un formulario dinámico en lugar de repreguntar por texto).
   - **Flujos paso a paso:** Presentes un plan, guía o lista de tareas ejecutable.
   - **Datos complejos / Geográficos:** Muestres direcciones, rutas, mapas, tablas o estadísticas.
3. **Restricción de texto simple:** Si la respuesta se resuelve con una explicación conceptual o un dato directo, NO invoques la herramienta. Responde únicamente con texto plano.
4. **Procesamiento de respuestas:** Cuando el usuario interactúe con el widget, recibirás un mensaje de entrada con los datos seleccionados. Procesa la respuesta de inmediato y confirma el resultado sin volver a renderizar el widget, a menos que se requiera una modificación explícita.') > 0;
