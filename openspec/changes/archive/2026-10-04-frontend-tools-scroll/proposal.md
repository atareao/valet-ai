# Proposal

## Why

Con las 12 tools registradas, la pestaña «Herramientas» renderiza una `List` de antd sin tope de
altura. El cuerpo del `Modal` de `SettingsDialog` crece con la lista y las últimas filas quedan fuera
de la ventana, sin forma de alcanzarlas. No hay scroll ni altura máxima.

## What Changes

- **`frontend` (ADDED)**: requisito de que el panel `ToolsTab` envuelva la lista en un contenedor con
  altura máxima relativa al viewport (`60vh`) y `overflow-y: auto`, de modo que las filas que no caben
  se alcancen desplazándose y el `Modal` no se desborde.

## Capabilities

### New Capabilities

- Ninguna.

### Modified Capabilities

- `frontend` (se añade un requisito; no se tocan los existentes).

## Impact

- Solo `frontend/src/components/ToolsTab.tsx` y sus tests. **Sin cambios de backend.**
- El scrollbar ya está estilizado (6px, oscuro) en `frontend/src/global.css`.
