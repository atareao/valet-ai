# Proposal: Diagnóstico del 401 en la ingesta de notificaciones

## Why

Cuando `POST /api/notifications` rechaza un token, responde `401` con el **cuerpo vacío**. Desde fuera
es indistinguible «me olvidé la cabecera», «el esquema no es `Bearer`» y «el token no corresponde a
ninguna clave», y en el registro del servidor no queda **nada**: el `401` es mudo. En la verificación
en producción del change `notification-ingest` se gastaron dos intentos ciegos contra un `401` sin
cuerpo antes de dar con un token correcto. Este change hace visible el motivo.

## What Changes

- **El `401` de la ingesta SHALL llevar cuerpo JSON** `{"error":"unauthorized"}`, para que un emisor
  sepa que el fallo es de autenticación y no un error de la aplicación.
- **El middleware SHALL registrar el motivo de la denegación** en el log —cabecera ausente, esquema
  incorrecto o clave desconocida o revocada— **sin distinguir hacia fuera** la clave desconocida de
  la revocada, que comparten la misma respuesta.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `notifications`: el requirement de autenticación de la ingesta (`La ingesta SHALL exigir un token de
  aplicación válido`) pasa a exigir el cuerpo JSON y el registro del motivo, con dos escenarios nuevos.

## Impact

- **Backend:** `src/middleware/auth.rs` (`require_ingest_key`) únicamente.
- **Tests:** `tests/api/notifications.rs` gana aserciones de cuerpo y de `Content-Type`; los cuatro
  tests de `401` existentes ya sólo comprueban el estado, así que no se rompen.
- **Spec:** delta **MODIFIED** en `notifications`, sin capabilities nuevas.

## Non-goals

- **No se tocan los `401` de sesión OIDC.** El cuerpo JSON es sólo para la ingesta; las rutas con
  sesión siguen como están.
- **No se distingue hacia fuera** clave desconocida de revocada: hacia fuera las dos son el mismo
  `401` idéntico. La distinción vive sólo en el log.
- **No se añade rate limiting** ni se toca la persistencia, la validación del cuerpo ni el despacho.
- **No se toca `docker-compose.prod.yml`.**
