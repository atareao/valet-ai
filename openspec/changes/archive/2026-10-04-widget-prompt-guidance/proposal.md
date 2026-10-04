# Proposal

## Why

En las pruebas reales contra el contenedor (`deepseek/deepseek-v4.1-flash`), el uso de los widgets es
irregular:

- En varios turnos el modelo **narra** el widget («Aquí tiene… ¿le aparece?») sin llegar a invocar
  `render_widget`: el stream solo trae `chunk` y `done`, ningún `tool_call` ni evento `widget`.
  Afirmar que lo ha mostrado sin invocarlo es, de hecho, inventar información.
- Cuando sí lo invoca, no hay criterio explícito de **cuándo** usarlo: a veces responde en texto lo que
  pediría un widget.

El esquema de `data` ya quedó resuelto en `render-widget-schema` (vive en la `description` de la tool y
funciona), así que el hueco que queda es de **comportamiento**, no de formato. Y el prompt de
personalidad (`settings.system_prompt`) no dice nada específico sobre widgets: solo una regla genérica
de «usa las herramientas de forma proactiva».

## What Changes

- **`db/schema` (ADDED)**: una migración **aditiva e idempotente** anexa al final de
  `settings.system_prompt` una sección `# Instrucciones de Interfaz y Widgets Interactivos` que instruye a invocar
  `render_widget` (no narrarlo), sus criterios de activación, la restricción de texto simple y el
  procesamiento de la respuesta sin repintar. Solo anexa si la sección aún
  no está; nunca borra el texto existente.
- **`db/schema` (MODIFIED)**: se precisa el requisito de siembra para acotar la regla «no sobreescribir
  valores no vacíos» a la migración de siembra, y permitir explícitamente que una migración aditiva
  posterior **anexe** contenido sin eliminar las personalizaciones del usuario.

**Fuera de alcance (decidido con el usuario):**
- No se compone la guía en código (ruta A descartada): el contenido vive en el `system_prompt`, que
  sigue siendo visible y editable desde Ajustes.
- No se toca la `description` de `render_widget` (el esquema de `data` ya está ahí y funciona).
- Sin cambios de frontend.

## Capabilities

### New Capabilities

- Ninguna.

### Modified Capabilities

- `db/schema` (se añade un requisito y se modifica otro).

## Impact

- **Backend**: una migración nueva (`migrations/20261004000001_widget_prompt_guidance.sql`). Sin cambios
  de lógica ni de tipos: el orquestador ya lee `settings.system_prompt` en cada turno.
- **Tests**: `tests/db/migrations.rs` (escenarios de anexado, preservación e idempotencia).
- **No** se toca `docker-compose.prod.yml`, ni el `system_prompt` de instalaciones personalizadas más
  allá de anexar al final.
