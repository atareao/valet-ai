# Design: De «Guardar» a «Abrir» en el `LocationWidget`

## Context

Ver `proposal.md` — Why. Lo que condiciona el diseño:

- El widget es una pieza de la PWA de Valet (`manifest.webmanifest` con `display: standalone`), que se sirve por HTTPS y se usa sobre todo desde el móvil. No hay envoltorio nativo: lo único que se puede hacer es entregar al sistema operativo un destino que él sepa atender.
- `App.tsx` usa `darkAlgorithm` y el mapa ya lleva un filtro CSS oscuro; el widget conserva su carga diferida y su chunk propio.
- El front no husmea la plataforma en ningún otro sitio: esta sería la primera vez.
- El patrón para sacar configuración fuera del componente ya existe: `locationTiles.ts` se creó para no romper la regla de Fast Refresh de `only-export-components`. La resolución de enlaces sigue ese mismo patrón.
- `LocationWidget` es hoy el único widget que emite una acción hacia el backend además de `QuickForm` y `Checklist`.

## Goals / Non-Goals

**Goals:**

- Que el punto llegue a la aplicación de mapas del dispositivo con el menor número de suposiciones sobre el dispositivo del usuario.
- Que la elección de destino sea una función pura, testeable sin navegador y sin `jsdom` global.
- Que quitar «Guardar» no deje código muerto: ni `save_place`, ni `onAction`, ni el docstring que lo documenta.

**Non-Goals:**

- No se generaliza la detección de plataforma a un servicio compartido del front: se queda dentro del módulo de enlaces hasta que haya un segundo consumidor.
- No se configura el proveedor de mapas.
- No se convierten las coordenadas en una dirección ni al revés: no hay geocodificación en el cliente.

## Decisions

**D1 — `geo:` sin paquete en Android.**
Alternativa considerada: el `intent://maps.google.com/maps?q=…#Intent;scheme=https;package=com.google.android.apps.maps;S.browser_fallback_url=…;end;` con el que arrancó la conversación. Se descarta por tres motivos: fija un paquete que en Android sin servicios de Google no existe; el `browser_fallback_url` hay que codificarlo entero (si se deja en crudo, el parser del intent corta en el primer `&` y el fallback desaparece sin avisar); y el fragmento `#Intent;…;end;` es una superficie de error innecesaria. `geo:` es el esquema estándar de Android y delega la elección en el sistema, que es exactamente lo que se quiere.

**D2 — iOS usa `https://maps.apple.com/`, no `maps://`.**
`maps://` no tiene plan B si Apple Maps está deshabilitado: la navegación muere en silencio. La URL `https` se resuelve por universal link y degrada a la web de Apple Maps. Contrapartida asumida: Apple no documenta cómo nombrar un pin anclado a unas coordenadas, así que **en iOS la etiqueta no viaja**; el pin queda en el punto exacto, sin nombre. Es una asimetría real entre las dos plataformas y se documenta aquí en vez de inventar un formato.

**D3 — El resto de plataformas usa la URL oficial de Google Maps** (`search/?api=1&query={lat},{lon}`), la misma que ya usa «Cómo llegar». Se prefiere la forma documentada `api=1` a la heredada `maps?q=…(etiqueta)`: es la API estable y con las coordenadas basta para clavar el pin.

**D4 — «Abrir» se pinta como ancla y sin `target="_blank"`.**
Descartado `window.location.href = "…"` con un esquema no-http: en una PWA instalada, un esquema que el dispositivo no pueda atender deja la aplicación sin destino, sin botón de volver y sin mensaje — el fallo más caro del planteamiento inicial. Descartado también `window.open`, que con esquemas no-http devuelve `null` en varios navegadores y produciría un «No se pudo abrir el mapa» **falso**: la acción sí funciona y el usuario recibe un error. El ancla delega en el navegador el protocolo externo, conserva pulsación larga y copiar enlace, y no deja pestaña en blanco.

**D5 — La resolución de enlaces es una función pura en un módulo aparte.**
Mismo motivo que `locationTiles.ts`: sacar del componente todo lo que no es un componente para no romper la regla de Fast Refresh de ESLint, y poder testear la resolución con un `userAgent` de entrada en vez de manipular el global del navegador en cada caso. La frontera es nítida: el módulo no toca el DOM y el componente no sabe de plataformas.

**D6 — Detección de plataforma: `navigator.userAgent` más `navigator.maxTouchPoints` para iPadOS.**
`userAgentData` sería lo preferible, pero no está en Safari ni en Firefox y añadiría dos caminos con el mismo resultado. El caso que sí hay que cubrir es iPadOS 13+, que se declara `Macintosh`: se desambigua con `navigator.maxTouchPoints > 1`. Queda aislado en el módulo puro para que el sniffing no se propague por el front.

**D7 — Se elimina `onAction` del widget.**
Verificado antes de decidir: `save_place` no tiene ninguna referencia en `src/` ni en `migrations/`. El widget se queda sin acciones hacia el backend, así que el prop deja de usarse y hay que quitarlo también del destructuring: el lint del proyecto corre con `--max-warnings 0` y una variable sin usar rompe la build.

**D8 — La etiqueta se codifica con un codificador estricto RFC 3986, no con `encodeURIComponent` a secas.**
`encodeURIComponent` deja sin escapar `( ) ! ' *`. En un destino `geo:{lat},{lon}?q={lat},{lon}({etiqueta})` un `)` crudo procedente de la etiqueta cierra el paréntesis antes de tiempo y la etiqueta se corta —o directamente se pierde— sin ningún aviso. La etiqueta se codifica, por tanto, con el juego estricto: `encodeURIComponent` más el escapado de esos cinco caracteres a su forma porcentual. `Café (esquina)` sale como `Caf%C3%A9%20%28esquina%29`, con los paréntesis del destino como únicos estructurales. Es la clase de fallo que no se ve en un test con una etiqueta simple (`Oficina central`) y sí con una real.

## Risks / Trade-offs

- **Dispositivo sin ninguna aplicación que atienda `geo:`** → es el comportamiento estándar del sistema y no se inventa un fallback propio. «Copiar coordenadas» y «Cómo llegar» (que abre Google Maps web) siguen disponibles como salidas.
- **Navegadores embebidos** (webview de WhatsApp, Telegram, Instagram) que bloquean esquemas externos → al usar un ancla en la misma pestaña, el peor caso es que no ocurra nada; la PWA no se queda rota, que es lo que se quería evitar.
- **iPadOS se declara `Macintosh`** → cubierto por `navigator.maxTouchPoints`.
- **«Abrir» y «Cómo llegar» coinciden en escritorio y en Android** (Google Maps, misma consulta) → aceptado explícitamente como non-goal del proposal; la distinción real es app nativa frente a web.
- **`static/` es una copia versionada de la build y no hay receta que la sincronice** → el `Dockerfile` la regenera desde `frontend/dist`, pero `cargo run` y `tests/api/frontend_serving.rs` sirven la copia del repositorio. Sin resincronizar, el contenedor mostraría «Abrir» y la ejecución local seguiría mostrando «Guardar». Es deriva preexistente que este cambio destapa; se resuelve como tarea explícita.

## Migration Plan

Sin migración de datos y sin estado persistido que deshacer. El despliegue es el habitual (`just dev` para el contenedor de trabajo, `just deploy` para la imagen publicada). Rollback: revertir el commit; el único artefacto con estado es la copia versionada de `static/`, que vuelve con el propio revert.
