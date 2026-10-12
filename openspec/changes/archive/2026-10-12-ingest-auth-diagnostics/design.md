# Design

## Cuerpo JSON en vez de cuerpo vacío

El `401` se produce en el middleware, antes de tocar el handler. Hoy devuelve
`StatusCode::UNAUTHORIZED.into_response()`, que lleva cuerpo vacío. Se sustituye por una respuesta
`401` con `Content-Type: application/json` y cuerpo `{"error":"unauthorized"}`, construida en el
mismo middleware. El cuerpo es genérico a propósito: no revela si la clave existe, está revocada o
nunca se presentó.

## Tres motivos, no cuatro

`find_by_hash` devuelve `Ok(None)` tanto para una clave **desconocida** como para una **revocada**
(la revocada no autentica y no mueve `last_used_at`). Distinguirlas en el log exigiría un método de
repositorio nuevo sólo para el diagnóstico. No compensa: el log necesita responder «¿el token llegó
siquiera?» y «¿coincidió con alguna clave?», y eso se cubre con tres motivos —
`missing_authorization`, `malformed_authorization` y `unknown_or_revoked_key`—. Hacia fuera los tres
comparten el mismo `401`.

## El token nunca se registra

El log registra el **motivo**, no el token ni su prefijo. Es la misma regla que ya rige el resto de la
ingesta: de una clave sólo se registran `id` y `name`, nunca el material que la autentica.
