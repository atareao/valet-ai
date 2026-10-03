# Proposal

## Why

El backend ya expone el catálogo de herramientas (`GET /api/tools`) y su habilitación
(`PUT /api/tools/{id}/toggle`), y el registry refleja el cambio en caliente (`tools/registry`). Pero el
frontend **no tiene ninguna UI** para ello: hoy la única forma de activar o desactivar una tool es
llamar a la API a mano. Se añade una pestaña «Herramientas» en Settings para gestionarlo desde la
interfaz.

## What Changes

- **Nueva pestaña «Herramientas»** en `SettingsDialog`: lista todas las tools con nombre, descripción
  y un `Switch` que refleja su estado `enabled`; al cambiarlo llama a `PUT /api/tools/{id}/toggle` y
  actualiza el estado. Estado de carga y de error integrados con el resto del diálogo.
- **Cliente API**: nuevos métodos `getTools()` y `toggleTool(id)`, y el tipo `Tool`
  (`{ id, name, description, enabled }`).
- **Requisito de layout MODIFIED**: el requisito «SettingsDialog SHALL fit all its top-level tabs
  without overflow» pasa de **seis** a **siete** pestañas superiores y el ancho del modal se amplía
  (900 → 1000) para que quepan en una fila.
- **Tests**: se actualiza el test que fija «exactly six top-level tabs» a siete (con «Herramientas»)
  y se añaden los de la nueva pestaña.

## Capabilities

### Modified Capabilities

- `frontend`: nueva pestaña «Herramientas» en `SettingsDialog` y ajuste del requisito que fija el
  número de pestañas superiores y el ancho del modal.

## Impact

- Frontend: `frontend/src/components/SettingsDialog.tsx`, `frontend/src/api/client.ts`,
  `frontend/src/types/index.ts`, `frontend/src/test/SettingsDialog.test.tsx`.
- Sin cambios de backend: los endpoints y el registry ya existen y están especificados.
- Fuera de alcance: ordenar/agrupar tools, buscador, tooltips, o editar descripciones.
