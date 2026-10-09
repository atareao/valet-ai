# Tasks

## 1. Frontend — RED

- [x] 1.1 Extender `LoginPage.test.tsx`: el logo de Valet se renderiza como `img[src*='valet-icon']`, **no** hay emoji `💬`, el logo mide 120×120 y el contenedor raíz tiene fondo oscuro; se conserva el test de navegación a `/api/auth/login`.
- [x] 1.2 `npx vitest run` → los tests nuevos fallan y el resto sigue verde (RED).

## 2. Frontend — GREEN

- [x] 2.1 Reescribir `LoginPage.tsx`: contenedor a `100vh` con fondo `#000000`, logo `valet-icon.svg` a 120 px y botón "Iniciar sesión" con `handleLogin`. Sin título ni texto.
- [x] 2.2 `npx vitest run` verde; `npx tsc --noEmit` y `npm run lint`.

## 3. Revisión y cierre

- [x] 3.1 Revisión `react-reviewer`.
- [x] 3.2 Verificación visual (fondo oscuro + logo + botón) y `just check-all`.
- [x] 3.3 PR a `development` y `openspec archive login-dark-ux`. (archivado como `2026-10-07-login-dark-ux`; delta consolidado en `openspec/specs/frontend/spec.md`)
