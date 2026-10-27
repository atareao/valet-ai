# Change: Reorganizar ajustes de integraciones en sub-pestañas y renombrar «API Keys» a «Claves»

## Why

Hoy las credenciales de seis servicios (OpenWeatherMap, Google Places, Brave Search, apimail,
Miniflux y, en su bloque propio, ZenNotes) viven mezcladas con las claves de ingesta de la
aplicación dentro de una única pestaña llamada «API Keys». Eso confunde dos cosas distintas:

- **credenciales de integraciones**: claves de servicios de terceros que la app consume
  (OpenWeatherMap, Google Places, Brave Search, apimail, Miniflux, Strava y el token de ZenNotes);
- **credenciales de ingesta**: claves que la app **emite** para que agentes externos puedan
  ingerir eventos por la API (`IngestKeysSection`).

Además, el diálogo no dice en ninguna parte **qué integraciones están configuradas**: para
saberlo hay que abrir la pestaña y leer campo a campo.

## What changes

- La pestaña **Integraciones** pasa a organizarse en **sub-pestañas horizontales** (antd `Tabs`
  anidado), en este orden exacto: **Strava, OpenWeatherMap, Google Places, Brave Search, apimail,
  Miniflux, ZenNotes**.
- Cada sub-pestaña tiene **su propio botón «Guardar»** (no un guardar común). Strava y ZenNotes
  conservan sus flujos actuales (`StravaIntegration`, `ZennotesSection`); las cinco sub-pestañas de
  claves reutilizan el mismo `Form` (`settingsForm`) y `handleSettingsSubmit`, sin cambiar las
  claves de ajuste ni los endpoints.
- La pestaña **«API Keys» se renombra a «Claves»** y se queda **solo** con la gestión de claves de
  ingesta (`IngestKeysSection`). Deja de mostrar campos de API keys, las URL base de apimail y
  Miniflux y el token de ZenNotes.
- Cada integración muestra un **indicador de configuración** visible en la etiqueta de su
  sub-pestaña desde el primer render del panel (no depende de abrir la sub-pestaña) y en una línea
  de estado dentro del panel, con texto accesible (no solo color):
  - Strava → «Conectada» / «No conectada», de `GET /api/strava/status`;
  - OpenWeatherMap, Google Places, Brave Search, apimail y Miniflux → «Configurada» si la clave
    correspondiente tiene valor no vacío en `settings`, «Sin configurar» si no (para apimail y
    Miniflux el criterio es la API key; la URL base tiene valor por defecto);
  - ZenNotes → «Configurado» / «Sin configurar» según `configured` de `GET /api/zennotes/status`.
- Se actualizan los tests de `SettingsDialog.test.tsx` para navegar por Integraciones → sub-pestaña
  en lugar de por «API Keys», y para comprobar que «Claves» solo contiene claves de ingesta.
- El `Modal` de ajustes usa **`destroyOnHidden`**: al cerrar se desmonta todo su contenido y, al
  reabrir, las pestañas vuelven a su estado inicial (pestaña superior en «Perfil» y sub-pestaña de
  Integraciones en «Strava») y se releen los estados remotos, con **una lectura por apertura** de
  `GET /api/strava/status` y `GET /api/zennotes/status`. El `package.json` sube `antd` a
  `^5.25.0`, primera versión con `destroyOnHidden`; en anteriores la prop se ignora en silencio y
  el contenido sobrevive al cierre.

En la spec de `frontend`, el requisito «SettingsDialog SHALL display API Keys tab» se **elimina**
(sus escenarios describían campos que se mudan) y se sustituye por cuatro requisitos añadidos: la
pestaña «Claves» solo con claves de ingesta, las sub-pestañas de «Integraciones», los indicadores de
configuración y la cobertura de tests alineada. Se usa REMOVED + ADDED en lugar de MODIFIED porque
un bloque MODIFIED tiene que conservar todos los escenarios vigentes y aquí dejan de aplicar.

**Sin cambios de backend, de endpoints ni de claves de ajuste.** `SENSITIVE_KEYS` sigue igual y
las claves de OpenWeatherMap, Google Places, Brave Search, apimail y Miniflux siguen llegando en
`GET /api/settings`, de donde el frontend deriva el estado «configurada».

## Impact

- `frontend/src/components/SettingsDialog.tsx` — reestructuración de las pestañas `api-keys`
  (renombrada a «Claves») e `integrations` (sub-pestañas), y derivación del estado de configuración.
- `frontend/src/components/` — posiblemente nuevos componentes de sección por clave
  (`WeatherIntegration.tsx`, `PlacesIntegration.tsx`, `BraveSearchIntegration.tsx`,
  `ApimailIntegration.tsx`, `MinifluxIntegration.tsx`), siguiendo el patrón de
  `StravaIntegration.tsx` / `ZennotesSection.tsx`.
- `frontend/src/test/SettingsDialog.test.tsx` — los tests que abren «API Keys» y los que listan
  etiquetas de campos de credencial.
- Comentarios JSDoc de `frontend/src/components/IngestKeysSection.tsx` y
  `frontend/src/components/ZennotesSection.tsx`, que mencionan la «pestaña API Keys».
- Sin migraciones, sin cambios en `src/` (Rust) y sin cambios en `SENSITIVE_KEYS`.
