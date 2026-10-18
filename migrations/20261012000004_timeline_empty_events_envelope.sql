-- El prompt del extractor pide un envoltorio vacío que su propio formato exige.
--
-- El seed de `20261011000001_timeline_fact_integrity.sql` cerró su lista de
-- reglas con «Si el lote no contiene ningún hecho, devuelve una lista vacía»,
-- mientras el bloque «Formato de salida (solo JSON)» exige el objeto
-- `{"events": [...]}`. La prosa y el contrato de salida se contradicen. Medido
-- con los modelos reales: mistral obedece la frase y emite un `[]` pelado, que el
-- parser rechaza como `NotJson`; es el 35 % de los errores del extractor en
-- producción.
--
-- Esta migración no reescribe el prompt: solo sustituye esa frase por el
-- envoltorio vacío `{"events": []}`, de modo que el caso «lote sin hechos» valga
-- exactamente lo mismo que el contrato general. Se aplica con el idioma de las
-- migraciones hermanas (`20261012000003_pendientes_notifications_prompt.sql`):
-- idempotente, solo si el valor sigue siendo byte a byte el sembrado, y rellena
-- la clave si falta, sin pisar nunca un prompt personalizado.
--
-- Por qué cada bloque:
--
--   1) Promueve el prompt al texto corregido, pero solo cuando su valor es
--      todavía el de `20261011000001`. Una edición del usuario se deja intacta.
--
--   2) Siembra el texto corregido cuando la clave `timeline_prompt` está ausente,
--      para una base de datos donde el prompt no llegó a sembrarse.

-- 1) Solo el valor sembrado exacto se sustituye; un prompt editado se respeta.
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
- Si el lote no contiene ningún hecho, devuelve {"events": []}.

Formato de salida (solo JSON):
{"events": [{"source": 3, "category": "sport", "fact": "Corrí 10 km por la vía verde", "timestamp": "2026-10-11T10:21:00+00:00", "location": "vía verde"}]}', updated_at = datetime('now')
 WHERE key = 'timeline_prompt' AND value = '# EXTRACTOR DE HECHOS CRONOLÓGICOS
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
{"events": [{"source": 3, "category": "sport", "fact": "Corrí 10 km por la vía verde", "timestamp": "2026-10-11T10:21:00+00:00", "location": "vía verde"}]}';

-- 2) Siembra el texto ya corregido cuando la clave no existe.
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
- Si el lote no contiene ningún hecho, devuelve {"events": []}.

Formato de salida (solo JSON):
{"events": [{"source": 3, "category": "sport", "fact": "Corrí 10 km por la vía verde", "timestamp": "2026-10-11T10:21:00+00:00", "location": "vía verde"}]}', datetime('now')
WHERE NOT EXISTS (SELECT 1 FROM settings WHERE key = 'timeline_prompt');
