# Design

## El `[]` se confundía con un error

`extract_json_object` exige un **objeto** JSON: un `[]` de dos tokens (la respuesta «ningún hecho»)
cae por la rama que busca `{`…`}` y termina en `NotJson`. El resultado es `status = 'error'` para una
respuesta que es correcta. La tolerancia consiste en aceptar un array vacío como «cero hechos» **antes**
de buscar el objeto, de modo que el síntoma desaparezca del log y la estadística no mienta.

## Reintentar antes de rendirse

Un fallo de forma de verdad (`MissingEvents`, o contenido que no es JSON) no se arregla tolerando: se
arregla **volviendo a preguntar**. El reintento vive dentro de la misma pasada y respeta el «no
abortar» de la spec: agota un número acotado de intentos y solo entonces registra el error y sigue. No
se toca `is_indexed`, porque la pasada no se bloquea.

## Por qué volver a DeepSeek, con el razonamiento apagado

El extractor quedó en `mistralai/mistral-small-24b-instruct-2501`, que es quien devuelve las respuestas
de dos tokens. El proyecto ya usa `deepseek/deepseek-v4.1-flash` para el chat y el consolidador. Se le da
otra oportunidad al extractor con ese modelo, pero **con el razonamiento desactivado**: el transporte ya
traduce `ReasoningSpec::Off` a `{"enabled": false}` en el cuerpo de OpenRouter
(`src/llm/openrouter.rs`), y `GENERATION_TIMELINE_REASONING` ya vale `off` en producción. Esto importa:
un modelo de razonamiento con el razonamiento encendido devolvería tokens de razonamiento y contenido
vacío —justo el síntoma que perseguimos—. El modelo se configura por entorno (`TIMELINE_MODEL`), así que
es un cambio operativo, no un PR de código.

## Cómo se medirá

Antes y después, sobre la base de datos de producción:

```sql
SELECT status, error_message, completion_tokens FROM llm_requests WHERE kind = 'timeline';
SELECT COUNT(*) FROM timeline_events;
```

Hipótesis: menos `NotJson`, más hechos. La medición decide si el reintento por código basta o si el
problema era del modelo.
