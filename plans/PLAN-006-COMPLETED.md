# PLAN-006 — `LocationWidget`: de «Guardar» a «Abrir»

## Contexto (lo que hay hoy)

- El widget vive en `frontend/src/components/widgets/LocationWidget.tsx` y ofrece tres acciones: «Guardar», «Cómo llegar» y «Copiar coordenadas», fijadas en la spec `openspec/specs/frontend/spec.md:1304`.
- «Guardar» emite `onAction("save_place", …)`. Ese `save_place` **no tiene ningún consumidor en el backend**: 0 referencias en `src/` (Rust) y 0 en `migrations/`. El `design.md` archivado del widget ya lo declaraba «turno de usuario, sin persistencia» y excluía la persistencia de lugares como non-goal.
- «Cómo llegar» abre `https://www.google.com/maps/search/?api=1&query=<lat>,<lon>` en pestaña nueva; no da indicaciones de ruta.
- Los tiles del mapa son OpenStreetMap sin API key, con atribución visible y filtro CSS para el tema oscuro (decisión deliberada, spec `frontend:1311`).
- `static/` está versionado en git y lo sirve `ServeDir::new("static")` (`src/lib.rs:538`). El `Dockerfile` regenera `/app/static` desde `frontend/dist` en cada build, pero **no existe ninguna receta que sincronice `frontend/dist` → `static/`**: `cargo run` local y `tests/api/frontend_serving.rs` comen de la copia versionada.
- Última verificación de la línea base (2026-10-17): `cargo test` 1423 passed / 0 failed; `vitest run` 61 ficheros / 433 tests; `tsc --noEmit` limpio; `eslint . --max-warnings 0` limpio.

## Tema 1 — Sustituir «Guardar» por «Abrir»

> **Estado: completado y archivado el 2026-10-18.** El change se archivó como
> `openspec/changes/archive/2026-10-18-location-widget-open-action/`: entró un requisito y salió otro
> en `openspec/specs/frontend/spec.md`, con los **11 escenarios** idénticos byte a byte. Queda
> **una sola cosa sin verificar**: la prueba en un Android real (tarea 6.3 del change), aplazada por
> decisión del usuario —hay precedente de aplazar la verificación en dispositivo cuando la feature
> está cubierta por tests (`AGENTS.md` §V, punto 7). El resto del plan está cerrado.

### Objetivo

Quitar del `LocationWidget` el botón «Guardar» —cuya acción no consume nadie— y poner en su lugar «Abrir», que lleve la ubicación a la aplicación de mapas del dispositivo resolviendo el destino por plataforma.

### Lo decidido (2026-10-17)

1. **Fuera «Guardar», dentro «Abrir».** El widget pierde su única acción hacia el backend y pasa a ser presentacional.
2. **Android: esquema `geo:` sin fijar paquete.** Se respeta la aplicación de mapas por defecto del usuario (o el selector del sistema) y no se asume que Google Maps esté instalado. Descartado el `intent://…package=com.google.android.apps.maps#Intent;…` propuesto inicialmente: exige `scheme=https` y un `S.browser_fallback_url` codificado entero —si no, el parser del intent corta en el primer `&`— y fija un proveedor concreto.
3. **iOS: URL de Apple Maps.** Descartado `maps://` por no tener fallback si Apple Maps está deshabilitado.
4. **Resto: URL oficial de Google Maps** (`search/?api=1&query=`), la que ya usa «Cómo llegar».
5. **El destino se construye con las coordenadas, nunca con la dirección.** El `title` (o, en su defecto, la `address`) viaja como etiqueta codificada donde el esquema lo permite, jamás como consulta de búsqueda: un `q=<nombre de sitio>` pasa por geocodificación y aterriza donde quiere.
6. **«Abrir» se pinta como ancla, sin `target="_blank"`.** Se descarta `window.location.href` con esquema no-http: en una PWA (`display: standalone`) un esquema que el dispositivo no pueda atender deja la aplicación en un callejón sin salida. El ancla conserva además pulsación larga y copiar enlace.
7. **«Cómo llegar» NO se convierte en indicaciones de ruta** («por ahora déjalo así», decisión del usuario). Consecuencia aceptada y anotada en el proposal como non-goal.
8. **Sin ajuste de proveedor de mapas preferido.** Anotado como tema pendiente.

### Alcance

- `frontend/src/components/widgets/LocationWidget.tsx` — quitar «Guardar», `handleSave`, `onAction` y el `save_place`; añadir «Abrir»; actualizar el docstring del componente, que hoy documenta `onAction("save_place", …)`.
- `frontend/src/components/widgets/locationLinks.ts` (nuevo) — resolución de enlaces por plataforma, función pura. Se saca del componente por el mismo motivo que `locationTiles.ts`: no romper la regla de Fast Refresh y poder testear sin tocar `jsdom` global.
- `frontend/src/test/LocationWidget.test.tsx` — fuera el test de `save_place` (línea 55); dentro los de «Abrir» por plataforma, codificación de etiqueta y ausencia de «Guardar». Hoy **no hay ni un test** de «Cómo llegar» ni de «Copiar coordenadas».
- `openspec/specs/frontend/spec.md` — el requisito «El widget LocationWidget SHALL mostrar una ubicación en un mapa con acciones» se **retira** y entra el sustituto «El widget LocationWidget SHALL mostrar una ubicación en un mapa con las acciones Abrir, Cómo llegar y Copiar coordenadas». No puede hacerse como `MODIFIED`: OpenSpec 1.13.2 (guard #1477) rechaza que un bloque `MODIFIED` omita escenarios que la spec actual tiene, y aquí desaparece el de «Guardar». El delta va, por tanto, como `## REMOVED Requirements` (solo el nombre más `**Reason**`) y `## ADDED Requirements`, que es el patrón que ya usa `llm-worker-resilience`. El encabezado retirado coincide **byte a byte** con el de la spec destino, así que el `archive` no tropezará.
- `static/` — resincronizar la copia versionada **excluyendo `.vite/`** (ver Riesgos: el `Dockerfile` lo elimina de la imagen).
- **Sin Rust, sin migraciones, sin claves nuevas en `settings`.** `src/tools/widget.rs` solo describe el esquema de `data`, no las acciones: no se toca.

### Mapa técnico

| Plataforma | Destino de «Abrir» |
|---|---|
| Android | `geo:{lat},{lon}?q={lat},{lon}({etiqueta codificada})` — sin `package` |
| iOS | `https://maps.apple.com/?q={lat},{lon}` — sin etiqueta (Apple no documenta nombrar un pin por coordenadas) |
| Resto | `https://www.google.com/maps/search/?api=1&query={lat},{lon}` |

Detección de plataforma por `navigator.userAgent`, con el caso de iPadOS que se declara `Macintosh` cubierto por `navigator.maxTouchPoints`. Es el primer uso de sniffing de plataforma en el front: queda aislado en el módulo puro para que no se propague.

### Tareas (TDD)

1. **RED — módulo puro:** tests de Android (`geo:`, sin paquete), iOS (Apple Maps) y resto (Google Maps); etiqueta codificada con espacios, acentos y paréntesis; sin etiqueta no se añaden paréntesis; `userAgent` de iPadOS resuelve como iOS.
2. **GREEN — módulo puro:** implementar `locationLinks.ts`; `npx tsc --noEmit` limpio.
3. **RED — widget:** no existe el botón «Guardar» y no se invoca `onAction`; «Abrir» apunta al destino resuelto por plataforma y no abre pestaña nueva; «Cómo llegar» y «Copiar coordenadas» siguen funcionando.
4. **GREEN — widget:** sustituir el botón, eliminar la acción, actualizar el docstring; suite del widget en verde; `grep` sin restos de `save_place` en `frontend/src/`.
5. **Refactor y build versionada:** `eslint . --max-warnings 0` limpio; reconstruir el frontend y sincronizar `static/`; verificar que `static/assets/LocationWidget-*.js` ya no contiene `save_place` y que el chunk con hash viejo ha desaparecido.
6. **Verificación y cierre:** `cargo test` (incluida `tests/api/frontend_serving.rs`, que sirve `static/`), `vitest run` completo, `openspec validate --strict`, prueba real desde el móvil contra el contenedor, revisión con `@react-reviewer` y `openspec archive`.

### Definición de Hecho (DoD)

- ✅ El widget muestra «Abrir», «Cómo llegar» y «Copiar coordenadas»; «Guardar» no existe.
- ⏳ aplazado «Abrir» desde Android abre el selector de aplicaciones o la app de mapas por defecto, y **no** una pestaña en blanco. ⏳ **aplazado**: es la tarea 6.3, la única prueba que no se ha podido hacer aquí.
- ✅ En iOS y en escritorio el destino es el correcto y contiene las coordenadas.
- ✅ Cero referencias a `save_place` en `frontend/src/`.
- ✅ `frontend/dist` y `static/` están sincronizados: la aplicación local y el contenedor muestran los mismos botones.
- ✅ `cargo test`, `vitest run`, `tsc --noEmit` y `eslint . --max-warnings 0` en verde, y `openspec validate location-widget-open-action --strict` OK.

### Resultado verificado (2026-10-18)

| Comprobación | Antes | Después |
|:--|:--|:--|
| `vitest run` | 61 ficheros / 433 tests | **62 / 466** |
| `cargo test` | 38 suites / 1423 | **38 / 1423 / 0 fallos** |
| `tsc --noEmit` · `eslint . --max-warnings 0` | limpios | **limpios** |
| `openspec validate location-widget-open-action --strict` | — | **valid** |
| `save_place` en el repositorio | 2 sitios vivos | **0** |
| Requisitos en `openspec/specs/frontend/spec.md` | 58 | **58** (uno sale, otro entra) |

### Riesgos / notas

- **Dispositivo sin ninguna app que atienda `geo:`.** No se inventa fallback: es el comportamiento estándar del sistema. Quedan «Copiar coordenadas» y «Cómo llegar» (Google Maps web) como salidas.
- **Navegadores embebidos** (webview de WhatsApp/Telegram) pueden bloquear esquemas externos. Al usar un ancla, la PWA no se queda rota.
- **Deriva preexistente de `static/`, medida el 2026-10-18.** `static/` se sincronizó por última vez el 2026-10-12 (commit `b678d53`) y desde entonces **7 commits** tocaron `frontend/src`; `frontend/dist` se reconstruyó el 16/10 y nadie lo copió. Faltaban chunks enteros —`NotificationsView`, `sqliteTimestamp`— y casi todos los hashes diferían (19 chunks frente a 22). La deriva era mayor que este widget: `cargo run` servía el frontend del 12/10 y el contenedor el del build actual. La resincronía mueve 22 ficheros de `static/`: 12 añadidos, 9 borrados y 1 modificado (`index.html`), y borra `static/assets/LocationWidget-BwFm_-vD.js`, que era **la última referencia viva a `save_place`** en el repositorio.
- **`static/` no tiene receta.** La sincronía se hace a mano; añadir una receta al `.justfile` es trabajo aparte.
- **`dist/.vite/manifest.json` no se versiona ni se sirve.** El `Dockerfile` ya lo borraba a propósito antes de copiar la imagen (`RUN rm -rf dist/.vite`, con el comentario de que no debe servirse con `ServeDir`). La copia versionada imita a la imagen, así que queda **excluida** de `static/`; una sincronía con `rsync` a secas la reintroduce y hay que borrarla.

## Temas pendientes

- **Convertir «Cómo llegar» en indicaciones de ruta reales** (`/maps/dir/?api=1&destination=`, `google.navigation:q=`, `daddr=` en Apple). Hoy «Cómo llegar» abre una búsqueda, así que en escritorio y Android «Abrir» y «Cómo llegar» acaban en el mismo sitio con la misma consulta. Aplazado por decisión del usuario.
- **Ajuste de proveedor de mapas preferido** (Google Maps / Apple Maps / OpenStreetMap / app del sistema) en Ajustes → Interfaz.
- **Receta de sincronía `frontend/dist` → `static/`** en el `.justfile`, para que la deriva deje de depender de que alguien se acuerde.
- **Arrastrado:** `plans/PLAN-004-PENDING.md` §4 (Observabilidad) sigue pendiente de decidir si se rebasa a un plan nuevo, ya que su contenido está construido sobre `llm_requests` y no sobre el `telemetry_events` que el plan describe.
- **Receta de sincronía con exclusión.** Si algún día se añade al `.justfile`, debe excluir `.vite/` del `rsync` (o borrarlo después), o reintroducirá el manifiesto en la copia versionada.
- **`encodeLabel` lanza `URIError` ante un surrogate huérfano** y no hay test de esa entrada. Inalcanzable hoy (serde rechaza esos escapes y el JSON viene de Rust), pero el módulo expone `label?: string` sin contrato que lo garantice y la excepción ocurriría durante el render, tumbando el widget entero. Aceptado sin acción; anotado para que no se redescubra.
- **Coordenadas fuera de rango.** El widget valida que `latitude`/`longitude` sean finitos, no que estén en rango: un `1e21` del LLM se interpola como `1e+21` en el destino. Aceptado sin acción.
- **Dos fallos preexistentes de `openspec validate --specs --strict`, ajenos a este change.** `spec/frontend/notifications` (el `Purpose` es el texto de relleno de un `archive` viejo) y `spec/infra` (los requisitos 14 y 15 no llevan SHALL/MUST). Comprobado que están ya en `HEAD` y que este change no los toca: `git status` de esos dos ficheros sale limpio. Quedan sin arreglar.
- **`role="img"` en el contenedor del mapa** (preexistente, no entra en este change): poda del árbol de accesibilidad los enlaces de atribución de OpenStreetMap que la spec exige visibles. Un `role="group"` o un `figure` con `caption` no los podaría.
