# Proposal

## Why

La pantalla que ve un usuario sin sesión es una vista blanca con el emoji `💬` como icono, ajena a la identidad visual de Valet y al tema oscuro que ya usa el resto de la aplicación (el tema es `darkAlgorithm` con `colorBgLayout: #000000`, pero `LoginPage` no pinta fondo y el navegador muestra su blanco por defecto). Antes de exponer la instancia, la pantalla de entrada debe verse intencionada: fondo oscuro y el logo real.

## What Changes

- La pantalla de login pasa a un contenedor a pantalla completa con **fondo oscuro** (`#000000`, el `colorBgLayout` del tema).
- Se sustituye el emoji `💬` por el **logo real de Valet** (`valet-icon.svg`), renderizado como imagen a **120 px**.
- Se **eliminan** el título "💬 Valet" y el texto "Inicia sesión para continuar": la pantalla queda **solo logo + botón** "Iniciar sesión".
- Sin cambios de comportamiento: el botón sigue navegando a `/api/auth/login`.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `frontend`: se actualiza el requisito "El frontend presenta una pantalla de login" para exigir fondo oscuro y el logo real de Valet a tamaño adecuado, en lugar del emoji de chat.

## Impact

- `frontend/src/components/LoginPage.tsx`: layout, fondo y logo.
- `frontend/src/test/LoginPage.test.tsx`: tests del logo y el fondo (TDD).
- Posible refuerzo en `frontend/src/global.css` (fondo del `body`).
- Sin cambios en backend ni en el flujo OIDC.
