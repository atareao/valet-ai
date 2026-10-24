# Change: El chat no razona por defecto

## Why

Los modelos razonadores (p. ej. `deepseek/deepseek-v4.1-flash`) **razonan por defecto** cuando la petición no lleva el campo `reasoning`: medido contra OpenRouter, una pregunta trivial pasó de 0,63 s y 0 tokens de razonamiento a 2,09 s y **34 tokens de razonamiento** solo por omitir el campo. Valet siembra `GENERATION_CHAT_REASONING` **vacío**, y vacío significa «no enviar el campo y dejar decidir al modelo»; los demás roles (colapso, fichas, consolidación y timeline) ya nacen en `off`, pero **el chat no**. El resultado es que el chat razona —y encarece y alarga cada turno— sin que el usuario lo haya pedido.

## What Changes

- **MODIFIED** el default de razonamiento del rol **chat**: pasa de vacío a `off` (tanto en el valor sembrado por migración como en el default de código del rol).
- **Nuevo** una migración que, para las bases ya existentes, cambia a `off` el `GENERATION_CHAT_REASONING` que esté **vacío** (el default anterior). Un valor explícito distinto SHALL respetarse.
- **MODIFIED** el `SettingsDialog`: el bloque de chat muestra `off` por defecto cuando el ajuste falta; un `default` (vacío) elegido explícitamente se respeta.
- El usuario sigue pudiendo **subir** el nivel de razonamiento desde Ajustes → Generación → Chat. Los demás roles no cambian.

## Capabilities

- **Modified Capabilities**:
  - `db/repos`: el default inicial de `GENERATION_CHAT_REASONING` pasa de vacío a `off`.
  - `orchestrator/agent`: el default de razonamiento del chat pasa a `off`.
  - `frontend`: el bloque de chat del SettingsDialog muestra `off` por defecto.

## Impact

- **Backend**: `src/generation.rs` (default del rol chat), nueva migración en `migrations/`, `src/db/repos/settings.rs` y sus tests.
- **Frontend**: `frontend/src/components/SettingsDialog.tsx` (valor por defecto del bloque de chat).
- **Datos**: la migración hace `UPDATE settings SET value='off' WHERE key='GENERATION_CHAT_REASONING' AND value=''`.
- **Fuera de alcance**: cambiar de modelo, tocar los otros roles, ni tocar producción ni `docker-compose.prod.yml`.
