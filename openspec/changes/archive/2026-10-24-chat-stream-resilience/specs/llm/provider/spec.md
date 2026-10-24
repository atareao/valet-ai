## ADDED Requirements

### Requirement: El streaming SHALL usar un timeout de inactividad, nunca un timeout total

`OpenRouterProvider::chat_stream()` SHALL NOT estar sujeto a un timeout **total** de reloj de pared que abarque la lectura del cuerpo. SHALL usar un **timeout de inactividad** —el tiempo máximo sin recibir bytes nuevos— de modo que una respuesta que sigue emitiendo fragmentos SHALL completarse aunque tarde, en total, más que ese tiempo. Una conexión que deja de producir fragmentos durante más que el timeout de inactividad SHALL fallar con un error de timeout y cerrar el stream.

#### Scenario: Una respuesta larga que fluye no se corta
**Given** una respuesta SSE que emite fragmentos de forma continua durante más de 120 s
**When** `chat_stream()` la consume
**Then** emite todos los `StreamEvent::Chunk` hasta `finish_reason`/`[DONE]`
**And** NO falla por timeout

#### Scenario: Un stream que se queda mudo falla por inactividad
**Given** una respuesta SSE que emite algunos fragmentos y después deja de producir bytes
**When** transcurre más que el timeout de inactividad sin nuevos bytes
**Then** `chat_stream()` emite `Err(LLMError::Timeout)`
**And** cierra el stream
