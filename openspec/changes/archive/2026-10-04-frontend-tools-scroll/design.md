# Design

## Context

- `ToolsTab` (`frontend/src/components/ToolsTab.tsx`) renderiza `<List>` de antd directamente.
- `SettingsDialog` lo embebe como `children` de la pestaña «Herramientas» dentro de un `Modal` de 1000 px.
- El proyecto ya define un scrollbar delgado y oscuro en `frontend/src/global.css`.
- Los tests del panel viven en `frontend/src/test/SettingsDialog.test.tsx` (mockean `useTools`), con
  jsdom, que **no calcula layout**: solo se puede verificar el estilo inline del contenedor.

## Goals / Non-Goals

- **Goal:** que la lista de tools tenga scroll vertical y una altura máxima, sin desbordar el Modal.
- **Non-Goal:** virtualizar la lista, paginar, reordenar tools, ni tocar el resto de pestañas.

## Decisions

- **Contener en `ToolsTab`, no en el `Modal`.** Así solo se desplaza la lista de tools y las demás
  pestañas (formularios, prompts) mantienen su comportamiento.
- **Estilo inline.** El contenedor usará `style={{ maxHeight: "60vh", overflowY: "auto", … }}`; al
  llevarlo inline, el test puede leer `element.style` en jsdom sin depender de CSS externo.
- **`60vh` como tope.** Deja visibles la cabecera y las pestañas del Modal y, con 12 filas, provoca
  scroll de forma fiable en pantallas habituales.
- **Sin scroll horizontal.** `overflow-x: hidden`; la descripción de antd ya ajusta el texto, y un
  scroll horizontal empeoraría la lectura en el `Modal`.

## Risks / Trade-offs

- [El scrollbar tapa ligeramente el `Switch`] → Un `padding-right` en el contenedor deja respirar la
  columna de acciones.
- [Alturas muy pequeñas de ventana] → `60vh` escala con el viewport; no se fija un `px` rígido.

## Migration Plan

Sin migración. Solo frontend.
