# PLAN-010 — Sesión duradera en el móvil (TTL configurable + renovación deslizante)

> Estado: **pendiente de aprobación** (documento de decisión). No se escribe código hasta aprobar el change proposal OpenSpec.
> Convención de repo: si se aprueba y ejecuta, este plan se renombra a `plans/PLAN-010.md`; al completarse, a `plans/PLAN-010-COMPLETED.md`.
> Origen: consulta del 2026-10-27 («en el móvil, ¿cómo podría hacer que la sesión durara mucho más?»). El usuario pidió dejarlo escrito y atender otra cosa antes de implementarlo.

## 0. Objetivo

Que la sesión de Valet en el móvil **deje de caducar a las 8 horas**, de modo que abrir la app no implique volver a pasar por PocketID salvo tras un abandono prolongado (semanas), y que un 401 transitorio no expulse al usuario a la pantalla de login en mitad de una conversación.

## 1. Diagnóstico: la duración está fijada en el código

| Dónde | Qué hay |
|---|---|
| `src/auth.rs:25` | `pub const SESSION_TTL_SECS: u64 = 60 * 60 * 8;` — **8 horas**, constante, sin lectura de entorno |
| `src/routes/auth.rs:157` | `exp: now + SESSION_TTL_SECS as usize` en los `Claims` del JWT de sesión |
| `src/routes/auth.rs:255-263` | `session_cookie()` con `max_age(Duration::seconds(SESSION_TTL_SECS as i64))`, `http_only(true)`, `secure(true)`, `SameSite::Lax`, `path("/")` |
| `src/middleware/auth.rs:59-62` | sólo `validate_session(...)`; **no reemite** cookie ni claims en las peticiones autenticadas |
| `src/config.rs:104-117` | el bloque `AUTH_*` no incluye ninguna variable de TTL |

Consecuencias verificadas del código (no supuestas):

1. **No existe ajuste ni variable de entorno.** No hay forma de alargar la sesión sin modificar el código. El `AUTH_*` completo es `AUTH_ENABLED`, `AUTH_ISSUER_URL`, `AUTH_CLIENT_ID`, `AUTH_CLIENT_SECRET`, `AUTH_REDIRECT_URL`, `AUTH_POST_LOGOUT_REDIRECT_URL` y `JWT_SECRET`.
2. **No hay deslizamiento.** La sesión nace con 8 h y muere a las 8 h aunque se use a diario. Quien entra a las 9:00 queda fuera a las 17:00.
3. **Segundo efecto, específico del móvil.** `frontend/src/api/client.ts:86` emite `valet:unauthorized` ante cualquier 401 fuera de `/auth/*`; `AuthProvider` lo traduce a `user: null` y `App.tsx` muestra la pantalla de login. En móvil el SSE se corta y reconecta al suspender la pestaña, así que un 401 transitorio (contenedor recreándose durante un despliegue, reconexión justo tras expirar) provoca la expulsión aunque el TTL nominal no haya vencido. Lo mismo ocurre al abrir una notificación push después de horas: se aterriza en el login.
4. **La purga del navegador no es la causa principal.** El service worker (`frontend/public/sw.js`) no registra handler de `fetch`: no cachea `/api/auth/me` ni sirve respuestas obsoletas. El problema es el TTL del servidor.

## 2. Opciones evaluadas

**A. TTL configurable por entorno (imprescindible, coste bajo).**
Leer `AUTH_SESSION_TTL_SECS` en `config.rs`, con **default 28800** para no cambiar el comportamiento actual, exponerlo en `AuthConfig` y usarlo en los dos puntos donde hoy aparece la constante (`Claims::exp` y `session_cookie`). En `co1` se fija `AUTH_SESSION_TTL_SECS=2592000` (30 días) en el `.env` del stack de Dockge y se recrea el contenedor. Resultado: entrar una vez al mes.

**B. Renovación deslizante (resuelve el problema de fondo).**
El `session_middleware`, cuando la sesión valida y le queda menos de una fracción del TTL (propuesta: 1/4), reemite cookie y claims actualizados, con **tope absoluto** (`now - iat < AUTH_SESSION_MAX_SECS`, propuesta 90 días) para que la sesión no sea inmortal. Combinado con A: mientras se use, no caduca; tras un abandono de 30 días, sí. Consideraciones de implementación:
- No reemitir en rutas exentas (`is_exempt`).
- El umbral temporal evita un `Set-Cookie` en cada petición del chat.
- El SSE funciona igual: la cookie se reemite en la petición que abre el stream, y `Set-Cookie` en la respuesta no rompe el streaming.

**C. Re-autenticación silenciosa (opcional, complementa a A+B).**
Que un 401 no muestre la pantalla de login sino que redirija a `/api/auth/login?return_to=<ruta>` (hoy `login` vuelve siempre a `/`, `src/routes/auth.rs:169`) y regrese solo si PocketID conserva su sesión. Convierte «diez días fuera» en un parpadeo. Requiere: parámetro nuevo con validación de destino (sólo rutas internas, para no ser un open redirect), guarda contra bucles y conservación mínima del estado de UI. Se aparca hasta ver el resultado de A+B.

**D. Revocación real con sesiones en base de datos (descartada por ahora).**
Tabla de sesiones, limpieza de expiradas y cambio del contrato de `/api/auth/me`. Es la única vía para **acortar** de verdad una sesión robada, pero no es lo que se pide y multiplica el alcance.

## 3. Recomendación

Implementar **A + B** en un único change (`session-longevity`), con TTL de 30 días, renovación deslizante al 25 % restante y tope absoluto de 90 días. En la práctica: el móvil deja de pedir login salvo tras un mes sin abrir la app, y los 401 transitorios dejan de expulsar al usuario porque la cookie siempre viaja fresca. C queda como mejora posterior si aún se ve la pantalla de login en algún caso real.

## 4. Coste de seguridad (explícito)

El JWT de sesión **no es revocable**: `logout` sólo borra la cookie (`src/routes/auth.rs:229`) y una copia robada sigue siendo válida hasta su `exp`. Con 30 días eso es una ventana larga. Mitigaciones que entran en el cambio:
- Tope absoluto con `iat` (90 días): la sesión no se renueva indefinidamente.
- Rotar `JWT_SECRET` invalida todas las sesiones de golpe, como palanca de emergencia ya existente.
Fuera de alcance: revocación selectiva (opción D).

## 5. Ficheros afectados (estimación)

| Fichero | Cambio |
|---|---|
| `src/config.rs` | leer `AUTH_SESSION_TTL_SECS` y `AUTH_SESSION_MAX_SECS`; añadirlas a las listas de variables que los tests limpian |
| `src/auth.rs` | `AuthConfig` gana los dos campos; la constante pasa a ser el valor por defecto documentado |
| `src/routes/auth.rs` | `Claims::exp` y `session_cookie` usan el TTL de configuración |
| `src/middleware/auth.rs` | reemisión deslizante con umbral y tope absoluto |
| `.env.example` | documentar las dos variables |
| `docker-compose.prod.yml`, `.env.j2` / `.env.prod.sample` | propagar las variables (no cambia comportamiento con el default) |
| `openspec/specs/oidc-auth/spec.md` (o spec del módulo correspondiente) | requisito de TTL configurable y renovación deslizante |
| `tests/api/auth.rs` | escenarios de expiración, renovación y tope absoluto |

## 6. Despliegue

1. Publicar la imagen con el cambio y recrear el stack desde Dockge (`~/docker/dockge/stacks/valet` en `co1`), que es el despliegue real de producción.
2. Añadir al `.env` del stack: `AUTH_SESSION_TTL_SECS=2592000` (y, si se quiere tocar el tope, `AUTH_SESSION_MAX_SECS`).
3. Verificar con `curl -i https://valet.territoriolinux.es/api/auth/me` tras autenticarse que el `Set-Cookie` lleva el `Max-Age` esperado.

## 7. Preguntas abiertas antes de aprobar

- TTL elegido: ¿30 días (propuesto) o 90?
- ¿Se incluye C (re-autenticación silenciosa) en el mismo change o se deja como change aparte?
- ¿Se quiere además un «cerrar sesión en todos los dispositivos» que rote `JWT_SECRET` desde Ajustes, o basta con la rotación manual?

## 8. Notas de plataforma (no verificadas en dispositivo)

- **iOS/Safari:** ITP limita a 7 días el almacenamiento escribible por script, pero las cookies `HttpOnly` de primera parte no deberían verse afectadas; sí las borra el usuario con «Borrar historial y datos» o la presión de almacenamiento. Añadir la app a la pantalla de inicio le da su propio almacén y evita la purga de pestañas de Safari. **No comprobado en dispositivo real: hay que verificarlo tras el despliegue.**
- **Android/Chrome:** las cookies persisten hasta que se limpian; el único límite es el TTL del servidor.
- Mientras PocketID conserve su sesión propia, «Entrar» es un toque más una redirección, sin volver a teclear credenciales.

## 9. Tareas (sin empezar)

- [ ] 1. Aprobar el alcance de este plan (A+B, TTL y tope).
- [ ] 2. Crear el change proposal `session-longevity` con delta de spec.
- [ ] 3. RED — tests de TTL configurable, renovación deslizante y tope absoluto.
- [ ] 4. GREEN — implementación mínima en `config.rs`, `auth.rs`, `routes/auth.rs`, `middleware/auth.rs`.
- [ ] 5. REFACTOR — `cargo clippy -- -D warnings`, `cargo fmt --check`.
- [ ] 6. Documentar variables en `.env.example` y en la spec de infraestructura.
- [ ] 7. Archivar el change y actualizar este fichero.
- [ ] 8. Release, publicar imagen y desplegar en `co1` con la variable en el `.env`.
- [ ] 9. Verificar `Set-Cookie` en producción y comprobar en un móvil real que la sesión sobrevive a un día completo.
