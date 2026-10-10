# Proposal

## Why

El modelo no «siente» el paso del tiempo: cuando una conversación se alarga, su única señal temporal es la sección de fecha/hora que manda el navegador —y solo si la manda—. Así, si un turno anterior leyó «Hoy es jueves 8 de octubre», el modelo puede seguir razonando «hoy = jueves 8» en turnos posteriores ya de otro día, y resolver «hoy», «ayer» o «mañana» contra una fecha caducada. Además, los mensajes del historial viajan **sin marca de tiempo**, de modo que el modelo no puede situar cuándo se dijo cada cosa.

## What Changes

- **Sección temporal siempre presente** en el mensaje de sistema, en cada turno, con el instante y la zona **efectivos del turno** y la instrucción de evaluar «hoy»/«ayer»/«mañana» respecto a ese timestamp.
- **La sección de ubicación pasa a llevar solo la ubicación** (coordenadas y, si se resuelve, el nombre); pierde su fecha y se omite cuando no hay coordenadas. La **sección temporal** pasa a cerrar el mensaje de sistema.
- **Marca de tiempo en cada mensaje conversacional**: los mensajes `user` y `assistant` del historial se prefijan con `[YYYY-MM-DD HH:MM]` en la zona del usuario, y el mensaje del turno actual con el instante efectivo. La marca **no se persiste** en la base de datos: solo viaja en la petición al LLM.
- **Resolución del instante y la zona por turno**: instante = timestamp del navegador si es válido, si no `Utc::now()`; zona = zona del navegador, si no `settings.timezone`, si no `Europe/Madrid`; zona inválida → UTC.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `orchestrator/agent`: el mensaje de sistema gana una sección temporal siempre presente que cierra la petición; la sección de ubicación pierde la fecha; los mensajes conversacionales (historial y turno) llevan marca de tiempo.

## Impact

- `src/tools/time_format.rs` — nuevos formateadores (`format_inline_timestamp`, `format_prompt_now`) sobre la conversión `chrono_tz` ya existente.
- `src/orchestrator/agent.rs` — resolución del instante/zona del turno, sección temporal, sección de ubicación sin fecha, prefijado del historial y del mensaje del turno; `compose_system_message` gana la sección temporal como última.
- Sin cambios de esquema, migraciones, API ni UI.
