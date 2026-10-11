-- Integridad de los hechos del timeline: sin duplicados, con fecha real y lugar.
--
-- La tabla `timeline_events` se desplegó el 2026-10-10 y en dos días acumuló un
-- 36 % de duplicados exactos, fechas tomadas de la mención y no del
-- acontecimiento, ningún lugar y hechos que eran fases de un mismo suceso.
-- Esta migración no la repara: la vacía. Los hechos son derivados —los escribió
-- el extractor, no el usuario— y el material crudo sigue en `messages`, así que
-- la tabla es reconstruible. Vaciarla elimina además el único paso frágil del
-- change (deduplicar antes de crear el índice) y deja el índice creable sin
-- condiciones.
--
-- Bloques, en este orden:
--
--   1) El vaciado de la tabla, antes de crear el índice.
--
--   2) La columna `location` (TEXT, admite NULL): el lugar del hecho, opcional.
--
--   3) El índice único por hecho normalizado y día. La pareja
--      (`lower(trim(fact))`, `substr(timestamp, 1, 10)`) es la identidad de un
--      hecho: el mismo texto el mismo día no puede repetirse. La normalización
--      solo pliega el ASCII (SQLite no quita tildes), pero es consistente porque
--      el mismo valor se normaliza igual al insertar y al comparar.
--
--   4) La actualización de los dos prompts sembrados. Solo se reemplazan cuando
--      su valor es todavía el de `20261010000004_timeline_prompts.sql`; una
--      personalización se respeta. Una clave ausente se rellena.

-- 1) Vaciar la tabla: los hechos son derivados y no hay nada que conservar.
DELETE FROM timeline_events;

-- 2) El lugar del hecho, opcional.
ALTER TABLE timeline_events ADD COLUMN location TEXT;

-- 3) La identidad de un hecho: mismo texto normalizado y mismo día.
CREATE UNIQUE INDEX IF NOT EXISTS ux_timeline_events_fact_day
    ON timeline_events(lower(trim(fact)), substr(timestamp, 1, 10));

-- 4a) El prompt del extractor: se actualiza solo si sigue siendo el sembrado.
UPDATE settings SET value = '# EXTRACTOR DE HECHOS CRONOLÓGICOS
Recibes un lote de mensajes numerados de una conversación. Cada línea empieza por su número, su fecha (`created_at`) y su rol. Extrae los hechos atómicos que el usuario ha vivido, hecho o decidido.

Reglas:
- Un hecho es **un acontecimiento del pasado**, completo y comprensible sin la conversación: incluye el objeto y, cuando la línea lo diga, la distancia, el importe o el lugar. «Corrí 10 km por la vía verde» es un hecho; «entrenamiento» no.
- **Un acontecimiento es un solo hecho, no sus fases.** Salir a correr, correr diez kilómetros y terminar el entrenamiento son un solo hecho, no tres: no extraigas el principio, el desarrollo y el final de lo mismo.
- NO extraigas órdenes dirigidas al asistente ni peticiones dirigidas a él («da por cumplido el recordatorio», «marca los correos como leídos»), ni estados transitorios («todavía no me fui a dormir»), ni opiniones, ni preguntas, ni planes futuros sin confirmar.
- `source` es el número de la línea de la que sale el hecho. Es obligatorio, tiene que ser uno de los números entre corchetes y **su rol ha de ser `user`**: los mensajes del asistente son contexto, nunca origen de un hecho.
- `timestamp` es la fecha y la hora en que ocurrió el hecho, en ISO 8601 con desfase horario (`2026-10-11T10:21:00+00:00`). Dedúcela del mensaje y del INSTANTE DE REFERENCIA que acompaña al lote; si el mensaje no permite saberla, omite la clave.
- `location` es el lugar donde ocurrió el hecho, tal y como aparezca en el mensaje (`Pulpería Terra Meiga`, `vía verde`). Si no se dice, omite la clave.
- NO repitas ningún hecho de la lista de hechos ya registrados que acompaña al lote, ni con las mismas palabras ni con otras.
- Elige la categoría de entre: sport, lifestyle, work, health, shopping, social, system.
- Ignora el ruido: saludos, agradecimientos, peticiones de formato y mensajes sin contenido factual.
- Si el lote no contiene ningún hecho, devuelve una lista vacía.

Formato de salida (solo JSON):
{"events": [{"source": 3, "category": "sport", "fact": "Corrí 10 km por la vía verde", "timestamp": "2026-10-11T10:21:00+00:00", "location": "vía verde"}]}', updated_at = datetime('now')
 WHERE key = 'timeline_prompt' AND value = '# EXTRACTOR DE HECHOS CRONOLÓGICOS
Recibes un lote de mensajes numerados de una conversación. Cada línea empieza por su número, su fecha (`created_at`) y su rol. Extrae los hechos atómicos que el usuario ha vivido, hecho o decidido.

Reglas:
- Un hecho es una sola acción o acontecimiento del pasado: «Fui al gimnasio», «Compré filtros de café», «Cerré el PR #168».
- NO extraigas preguntas del asistente, ni opiniones sin hecho, ni planes futuros sin confirmar.
- `source` es el número de la línea de la que sale el hecho. Es obligatorio y debe ser uno de los números que aparecen entre corchetes. No lo inventes.
- NO devuelvas ninguna fecha: la fecha del hecho la toma el sistema del mensaje de origen.
- Elige la categoría de entre: sport, lifestyle, work, health, shopping, social, system.
- Ignora el ruido: saludos, agradecimientos, peticiones de formato y mensajes sin contenido factual.
- Si el lote no contiene ningún hecho, devuelve una lista vacía.

Formato de salida (solo JSON):
{"events": [{"source": 3, "category": "sport", "fact": "Fui a correr 8 km por la vía verde"}]}';
INSERT INTO settings (key, value, updated_at)
SELECT 'timeline_prompt', '# EXTRACTOR DE HECHOS CRONOLÓGICOS
Recibes un lote de mensajes numerados de una conversación. Cada línea empieza por su número, su fecha (`created_at`) y su rol. Extrae los hechos atómicos que el usuario ha vivido, hecho o decidido.

Reglas:
- Un hecho es **un acontecimiento del pasado**, completo y comprensible sin la conversación: incluye el objeto y, cuando la línea lo diga, la distancia, el importe o el lugar. «Corrí 10 km por la vía verde» es un hecho; «entrenamiento» no.
- **Un acontecimiento es un solo hecho, no sus fases.** Salir a correr, correr diez kilómetros y terminar el entrenamiento son un solo hecho, no tres: no extraigas el principio, el desarrollo y el final de lo mismo.
- NO extraigas órdenes dirigidas al asistente ni peticiones dirigidas a él («da por cumplido el recordatorio», «marca los correos como leídos»), ni estados transitorios («todavía no me fui a dormir»), ni opiniones, ni preguntas, ni planes futuros sin confirmar.
- `source` es el número de la línea de la que sale el hecho. Es obligatorio, tiene que ser uno de los números entre corchetes y **su rol ha de ser `user`**: los mensajes del asistente son contexto, nunca origen de un hecho.
- `timestamp` es la fecha y la hora en que ocurrió el hecho, en ISO 8601 con desfase horario (`2026-10-11T10:21:00+00:00`). Dedúcela del mensaje y del INSTANTE DE REFERENCIA que acompaña al lote; si el mensaje no permite saberla, omite la clave.
- `location` es el lugar donde ocurrió el hecho, tal y como aparezca en el mensaje (`Pulpería Terra Meiga`, `vía verde`). Si no se dice, omite la clave.
- NO repitas ningún hecho de la lista de hechos ya registrados que acompaña al lote, ni con las mismas palabras ni con otras.
- Elige la categoría de entre: sport, lifestyle, work, health, shopping, social, system.
- Ignora el ruido: saludos, agradecimientos, peticiones de formato y mensajes sin contenido factual.
- Si el lote no contiene ningún hecho, devuelve una lista vacía.

Formato de salida (solo JSON):
{"events": [{"source": 3, "category": "sport", "fact": "Corrí 10 km por la vía verde", "timestamp": "2026-10-11T10:21:00+00:00", "location": "vía verde"}]}', datetime('now')
WHERE NOT EXISTS (SELECT 1 FROM settings WHERE key = 'timeline_prompt');

-- 4b) La guía de la skill: mismo trato que el prompt del extractor.
UPDATE settings SET value = '# SKILL ACTIVA: TIMELINE
- «¿Qué hice ayer / el martes / esta semana?» se responde con `timeline_get_events`, acotando el rango con `start` y `end` en ISO 8601.
- Los hechos son atómicos y ya están fechados: no los deduzcas de la memoria episódica ni los reconstruyas por similitud.
- Cada hecho lleva su categoría, su fecha y su lugar (`location`) cuando se conoce; muéstralos al presentarlo.
- Si el usuario te cuenta algo que hizo y quiere que quede registrado, anótalo con `timeline_add_event`, con su `location` si lo dice; omite el `timestamp` para usar la hora actual.
- Un hecho repetido el mismo día no se duplica: si `timeline_add_event` responde que ya estaba registrado, dilo y no lo repitas.
- Borra con `timeline_delete_event` solo cuando el usuario lo pida y te confirme cuál; localiza el `id` con `timeline_get_events` en el mismo turno.
- Presenta la información de forma cronológica y responde siempre en español.', updated_at = datetime('now')
 WHERE key = 'SKILL_TIMELINE_PROMPT' AND value = '# SKILL ACTIVA: TIMELINE
- «¿Qué hice ayer / el martes / esta semana?» se responde con `timeline_get_events`, acotando el rango con `start` y `end` en ISO 8601.
- Los hechos son atómicos y ya están fechados: no los deduzcas de la memoria episódica ni los reconstruyas por similitud.
- Si el usuario te cuenta algo que hizo y quiere que quede registrado, anótalo con `timeline_add_event`; omite el `timestamp` para usar la hora actual.
- Borra con `timeline_delete_event` solo cuando el usuario lo pida y te confirme cuál; localiza el `id` con `timeline_get_events` en el mismo turno.
- Presenta la información de forma cronológica y responde siempre en español.';
INSERT INTO settings (key, value, updated_at)
SELECT 'SKILL_TIMELINE_PROMPT', '# SKILL ACTIVA: TIMELINE
- «¿Qué hice ayer / el martes / esta semana?» se responde con `timeline_get_events`, acotando el rango con `start` y `end` en ISO 8601.
- Los hechos son atómicos y ya están fechados: no los deduzcas de la memoria episódica ni los reconstruyas por similitud.
- Cada hecho lleva su categoría, su fecha y su lugar (`location`) cuando se conoce; muéstralos al presentarlo.
- Si el usuario te cuenta algo que hizo y quiere que quede registrado, anótalo con `timeline_add_event`, con su `location` si lo dice; omite el `timestamp` para usar la hora actual.
- Un hecho repetido el mismo día no se duplica: si `timeline_add_event` responde que ya estaba registrado, dilo y no lo repitas.
- Borra con `timeline_delete_event` solo cuando el usuario lo pida y te confirme cuál; localiza el `id` con `timeline_get_events` en el mismo turno.
- Presenta la información de forma cronológica y responde siempre en español.', datetime('now')
WHERE NOT EXISTS (SELECT 1 FROM settings WHERE key = 'SKILL_TIMELINE_PROMPT');
