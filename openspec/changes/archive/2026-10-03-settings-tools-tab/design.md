# Design

## Context

`SettingsDialog` (`frontend/src/components/SettingsDialog.tsx`) monta sus pestañas con un `Tabs` de
antd (`width={900}`). Hoy tiene seis pestañas superiores: Perfil, Interfaz, Prompts, API Keys,
Memoria y Generación. El backend expone `GET /api/tools` (lista `{id,name,description,enabled}`) y
`PUT /api/tools/{id}/toggle` (devuelve la tool actualizada), y el registry se refresca en caliente.

## Goals / Non-Goals

- **Goal:** activar/desactivar cualquier tool desde Settings, viendo su estado real.
- **Non-Goal:** cambiar el comportamiento del backend ni el del registry.

## Decisions

- **Pestaña superior, no sub-pestaña.** Es una acción de administración global; encaja como séptima
  pestaña al mismo nivel que las demás. Alternativa descartada: sub-pestaña dentro de «Generación» o
  «API Keys» (escondería una acción primaria).
- **`Switch` por tool con la lista de antd.** Cada fila muestra nombre + descripción y un `Switch`
  ligado a `enabled`. Alternativa descartada: tabla (más pesada para 12 filas).
- **Estado local tras el toggle.** Se usa la `Tool` que devuelve el endpoint para actualizar la fila,
  sin recargar toda la lista (el backend ya devuelve el recurso). Si la llamada falla, el `Switch`
  vuelve a su valor previo y se muestra un aviso vía `App.useApp()` (convención de `ui-feedback`).
- **Recargar al abrir la pestaña.** El estado habilitado puede cambiar por otras vías; se carga al
  montar el panel (antd lo hace de forma perezosa), no `forceRender`.
- **Ampliar el ancho del modal a 1000 px.** Con siete etiquetas («Herramientas» es larga), 900 px
  arriesga el desplegable de desbordamiento de antd que el requisito prohíbe.

- **Hook `useTools` en vez de llamadas directas en el componente.** `SettingsDialog` ya consume hooks
  (`useProfile`, `useSettings`, `usePersistentMemory`) y sus tests mockean hooks, no `fetch`. Se crea
  `hooks/useTools.ts` que encapsula `api.getTools`/`api.toggleTool`; así el componente se testea aislado
  y el hook se testea contra el cliente API mockeado. Alternativa descartada: `fetch` directo en el
  componente (rompería la convención de tests del repo).

## Risks / Trade-offs

- [Deshabilitar una tool afecta al prompt en la siguiente petición] → Es el comportamiento deseado y
  reversible; no se pide confirmación (a diferencia de operaciones destructivas como borrar).
- [El ancho 1000 px en pantallas pequeñas] → antd ya limita el modal al viewport; se acepta.

## Migration Plan

Sin migración. Cambio puramente de frontend; el estado `enabled` ya vive en la tabla `tools`.
