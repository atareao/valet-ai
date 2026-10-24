# Design: El chat no razona por defecto

## Context

El razonamiento de cada rol de generación vive en `settings` bajo `GENERATION_<ROL>_REASONING` y se lee **en cada llamada** (`src/generation.rs`). El campo se codifica como cadena: vacío ⇒ no se envía el campo `reasoning` (decide el modelo); `off`/`none` ⇒ `ReasoningSpec::Off` (`{"enabled": false}`); un nivel ⇒ `ReasoningSpec::Effort(<nivel>)`. El rol chat se siembra con `GENERATION_CHAT_REASONING` **vacío** (migración `20261003000001_generation_settings.sql`) y su default de código también es `""` (`GenerationRole::Chat`). Los otros cuatro roles ya nacen en `off`.

Con un modelo razonador, «vacío» equivale a **razonar**: medido contra OpenRouter con `deepseek/deepseek-v4.1-flash`, omitir el campo produjo 34 tokens de razonamiento y ~2× de latencia en una pregunta trivial; enviar `{"enabled": false}` los dejó en 0.

## Goals / Non-Goals

**Goals**

- Que el chat **no razone** salvo que el usuario suba el nivel explícitamente.
- Que el cambio valga tanto para instalaciones nuevas como para las ya migradas.
- Que un valor explícito del usuario se respete.

**Non-Goals**

- Cambiar de modelo ni el modelo.
- Tocar los otros cuatro roles (ya están en `off`).
- Añadir o quitar opciones del selector de razonamiento.
- Tocar producción ni `docker-compose.prod.yml`.

## Decisions

### D1 — El default se cambia en tres capas a la vez

Para que «el chat no razona» sea cierto en todos los caminos:

1. **Valor sembrado**: la migración deja `GENERATION_CHAT_REASONING = off`.
2. **Default de código** (`GenerationRole::Chat` en `src/generation.rs`): de `""` a `"off"`, para cuando la clave falte por completo.
3. **UI** (`SettingsDialog`): el bloque de chat cae a `off` cuando el ajuste falta.

Las tres son necesarias: la 2 sola no basta porque la clave **existe** en la base (sembrada vacía) y `read_reasoning` lee la fila, no el default de código; y la 3 evita que la interfaz muestre un vacío engañoso.

### D2 — Una migración corrige el vacío anterior, respetando lo explícito

La base ya trae `GENERATION_CHAT_REASONING = ''` de la migración anterior. Una migración nueva hace `UPDATE settings SET value='off' WHERE key='GENERATION_CHAT_REASONING' AND value=''`, de modo que solo cambia el vacío (el default heredado) y **respeta** cualquier valor no vacío (`off`, `low`, `high`, …). No se edita la migración antigua: las migraciones aplicadas son inmutables.

### D3 — El vacío explícito sigue existiendo

`off` pasa a ser el **default**, no la única opción. Un usuario que quiera dejar decidir al modelo puede seguir eligiendo `default` (vacío) desde Ajustes; ese vacío seguirá significando `None` (no se envía el campo). El cambio solo altera el valor **inicial**, no la semántica de los valores.

En la **carga** del `SettingsDialog`, el bloque de chat usa `??` (no `||`): una clave **ausente** en `GET /settings` cae a `off`, pero un vacío **explícito** (`""`, la opción `default`) se respeta y se muestra como `default`. Así el escenario «vacío ⇒ `off`» se refina a «ausente ⇒ `off`»: quien eligió `default` a propósito lo conserva al reabrir Ajustes y, al volver a guardar sin tocar el selector, se envía vacío en vez de pisarse con `off`. El guardado ya usaba `??`, de modo que ambos caminos son coherentes y la elección no se pierde.

## Risks / Trade-offs

- Un usuario que hubiera dejado el chat en `default` (vacío) a propósito pasará a `off` tras migrar. Puede volver a elegir `default` desde Ajustes → Generación → Chat. Se acepta: el vacío era el default de fábrica, no una elección explícita en la inmensa mayoría de los casos.
- Apagar el razonamiento puede bajar la calidad en tareas que se beneficiaban de él. El usuario puede **subir** el nivel cuando quiera; el cambio no lo impide.
