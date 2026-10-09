# Design

## Contexto

- El tema ya es oscuro (`frontend/src/theme.ts`: `darkAlgorithm`, `colorBgLayout: #000000`), pero `LoginPage` no pinta fondo y `global.css` no asigna fondo al `body` → el navegador muestra blanco.
- El logo existe en `frontend/src/assets/valet-icon.svg` y se importa igual que en `MessageBubble.tsx` (`import valetIcon from "../assets/valet-icon.svg"`).

## Enfoque

- Contenedor `div` con `display:flex`, `alignItems:center`, `justifyContent:center`, `minHeight:100vh` y `background:#000000` (mismo valor que `colorBgLayout`).
- Logo: `<img src={valetIcon} alt="Valet" width={120} height={120} />`.
- Botón antd `type="primary" size="large"` con `handleLogin` (navega a `/api/auth/login`). **Sin título ni texto.**
- Se retiran `Typography`/`Space` si dejan de usarse.

## Tests (TDD)

- Extender `frontend/src/test/LoginPage.test.tsx`:
  - el logo se renderiza como `img[src*='valet-icon']` y **no** aparece el emoji;
  - el logo mide 120×120;
  - el contenedor raíz tiene fondo oscuro;
  - se mantiene el test de navegación a `/api/auth/login`.

## Alternativas descartadas

- Poner el fondo solo en `body` (`global.css`): se prefiere que el componente sea autosuficiente; el `body` puede reforzarse pero no es el único mecanismo.
