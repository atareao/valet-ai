# Change: La marca de tiempo del historial se limita a los turnos del usuario

## Why

El modelo **imita el formato de sus propios turnos**. Como el historial iba prefijado
también en los mensajes `assistant`, el LLM empezó a arrancar sus respuestas con
`[YYYY-MM-DD HH:MM]`; esa marca, persistida tal cual, aparecía en el chat y se
duplicaba en la petición siguiente (la del modelo más la que añadía el código).

## What Changes

- La marca inline `[YYYY-MM-DD HH:MM]` se aplica **solo** a los mensajes `user`
  (historial y turno actual). Los `assistant` viajan sin marca.
- Al montar el historial, el contenido de un `assistant` pierde una marca inicial
  heredada si la lleva: así el modelo deja de ver el patrón sin tocar la base de datos.
- La sección temporal añade la instrucción de no reproducir la marca.

## Impact

- Spec: `orchestrator/agent` — 2 requisitos modificados, 0 añadidos, 0 retirados.
- Código: `src/orchestrator/agent.rs`.
- Datos: **ninguno**. No se limpian las filas ya guardadas; el saneado ocurre en la petición.
- UI/API: sin cambios.
