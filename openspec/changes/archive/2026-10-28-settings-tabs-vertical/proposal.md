# Change: Pestañas verticales en el diálogo de ajustes

## Why

El diálogo de ajustes ha crecido hasta tener **diez pestañas superiores** —Perfil, Interfaz,
Prompts, Claves, Memoria, Generación, Skills, Enrutador de skills, Integraciones y
Notificaciones— y cinco grupos de pestañas anidadas (Prompts 6, Memoria 2, Generación 5, Skills
una por skill e Integraciones 8). En horizontal:

- **La fila superior ya no respira.** «Enrutador de skills» es una etiqueta larga y, con diez
  pestañas, la fila consume prácticamente todo el ancho del `Modal` (1000 px). Cualquier pestaña
  nueva —o un idioma con etiquetas más largas— cae en el desplegable de desbordamiento de antd,
  que esconde opciones en lugar de mostrarlas.
- **Las sub-pestañas repiten el problema un nivel más abajo.** Las ocho de «Integraciones» y las
  seis de «Prompts» ocupan su propia fila horizontal, de modo que en las pestañas con formulario
  largo el usuario mira dos filas de pestañas apiladas antes de llegar al primer campo.
- **Se desaprovecha el alto.** Los paneles son verticales por naturaleza (formularios en
  `layout="vertical"`, áreas de texto de diez filas); una lista lateral de pestañas ocupa el
  espacio que ningún campo usa y deja el contenido con todo el ancho.

## What Changes

Cambio **exclusivamente de orientación**: todas las pestañas del diálogo de ajustes pasan a
**verticales** (`tabPosition="left"`), sin tocar claves, orden, `forceRender`, etiquetas,
`aria-label`, formularios ni endpoints.

1. **Pestañas superiores** (`SettingsDialog.tsx`): las diez pasan a `tabPosition="left"`, de modo
   que el `tablist` queda como un índice lateral y el panel ocupa el resto del ancho del `Modal`.
2. **Pestañas anidadas**, todas a `tabPosition="left"`:
   - «Prompts»: System, Archivist, Collapse, Consolidator, Timeline y Journal (`SettingsDialog.tsx`).
   - «Memoria»: Episódica y Persistente (`SettingsDialog.tsx`).
   - «Generación»: Chat, Colapso, Fichas, Consolidación y Línea temporal (`SettingsDialog.tsx`).
   - «Skills»: una pestaña por skill (`SkillsTab.tsx`).
   - «Integraciones»: Strava, OpenWeatherMap, Google Places, Brave Search, apimail, Miniflux,
     ZenNotes y OpenTodo (`IntegrationsTab.tsx`).
3. **El ancho del `Modal` no cambia** (1000 px). Con la lista lateral, el panel pierde el ancho de
   la lista (unos 170 px) y gana el alto que antes gastaban las filas de pestañas; no hace falta
   reajustar el ancho ni añadir scroll nuevo.
4. **Specs**: se añade un requisito que fija la orientación vertical de **todas** las pestañas, se
   modifica el requisito de «Integraciones» —que hoy exige literalmente «sub-pestañas
   horizontales»— y se **retira** el requisito «SettingsDialog SHALL fit all its top-level tabs
   without overflow», cuya razón de ser (que siete pestañas quepan en una fila) desaparece con la
   orientación vertical y cuyo texto, además, estaba obsoleto: listaba «API Keys» y «Herramientas»,
   dos pestañas que ya no existen.
5. **Tests**: se actualiza la aserción que hoy comprueba que el `tablist` de Integraciones **no**
   tiene `aria-orientation="vertical"` y se añaden escenarios de orientación para los seis `Tabs`.

## Impact

- **Specs afectadas**: `frontend` (1 requisito añadido, 1 modificado, 1 retirado).
  `skill-router-ui` y `strava-ui` **no** cambian: describen estructura y contenido, no orientación.
- **Código afectado**: `frontend/src/components/SettingsDialog.tsx`,
  `frontend/src/components/SkillsTab.tsx`, `frontend/src/components/IntegrationsTab.tsx`;
  `frontend/src/test/SettingsDialog.test.tsx` (y, si alguno asertara orientación,
  `SkillsTab.test.tsx` / `IntegrationsTab.test.tsx`, que se revisarán).
- **Fuera de alcance**: `pages/StatsDashboard.tsx` también usa `Tabs`, pero no forma parte del
  diálogo de ajustes y se queda como está. `NotificationsTab`, `IngestKeysSection` y
  `RouterControl` no tienen pestañas propias.
- **Riesgo**: bajo. No hay cambios de contrato con el backend, ni de datos, ni de comportamiento:
  solo envuelve los mismos `items` en una orientación distinta. El único riesgo real es el
  desbordamiento vertical de la lista superior (diez pestañas) en pantallas bajas, que antd
  resuelve desplazando el propio `tablist`; el `Modal` de antd ya limita el alto al viewport.
- **Sin release propio**: entra en el próximo release sin cambio de versión por sí mismo.
