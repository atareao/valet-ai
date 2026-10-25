# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + specs + tasks) y **esperar el OK explícito** del usuario antes de escribir código o tests.

## 1. RED — timeout de inactividad en el cliente

- [x] 1.1 Test que falla: un stream SSE que sigue emitiendo fragmentos más allá de los 120 s totales NO se corta y llega a `finish_reason`/`[DONE]`.
- [x] 1.2 Test que falla: un stream que deja de producir durante más que el timeout de inactividad falla con un error de timeout y cierra el stream.
- [x] 1.3 Test que falla: `chat()` (no-streaming) sigue sujeto a un timeout total y falla si la respuesta se agota.

## 2. RED — configuración por entorno

- [x] 2.1 Test que falla: `OPENROUTER_TIMEOUT_SECS` y `OPENROUTER_STREAM_READ_TIMEOUT_SECS` sobrescriben los defaults.
- [x] 2.2 Test que falla: sin variables de entorno, los defaults son 120 s.
- [x] 2.3 Test que falla: un valor no numérico cae al default sin romper el arranque.

## 3. RED — texto parcial preservado

- [x] 3.1 Test que falla: si el stream falla tras emitir N fragmentos, el orquestador emite un evento de error que contiene el texto parcial acumulado.
- [x] 3.2 Test que falla: si el stream falla sin haber emitido ningún fragmento, el evento de error no lleva texto parcial.
- [x] 3.3 Test que falla: el error de timeout del stream queda registrado en `llm_requests` con un mensaje distinguible.

## 4. RED — frontend

- [x] 4.1 Test que falla: al recibir un evento de error con texto parcial, la burbuja del asistente mantiene el texto y muestra un aviso de interrupción.
- [x] 4.2 Test que falla: un error sin texto parcial muestra solo el aviso, sin burbuja vacía.

## 5. GREEN — backend

- [x] 5.1 `OpenRouterProvider` construye el/los clientes con `connect_timeout` + `read_timeout` (streaming) y timeout total (no-streaming).
- [x] 5.2 `OpenRouterConfig` lee los timeouts de entorno con defaults de 120 s.
- [x] 5.3 El orquestador incluye el `content_buffer` parcial en el evento de error del stream.
- [x] 5.4 `llm_requests` registra el fallo de timeout con mensaje distinguible.

## 6. GREEN — frontend

- [x] 6.1 El hook/burbuja mantiene el texto parcial recibido en el evento de error y muestra el aviso.

## 7. REFACTOR

- [x] 7.1 `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
- [x] 7.2 `npx tsc --noEmit`, `npm run lint`, `npx vitest run`.
- [x] 7.3 Documentar las nuevas variables de entorno.

## 8. Verificación y cierre

- [ ] 8.1 Prueba manual en producción (co1) con una pregunta de respuesta larga: no se corta ni desaparece.
- [ ] 8.2 `openspec archive chat-stream-resilience`.
