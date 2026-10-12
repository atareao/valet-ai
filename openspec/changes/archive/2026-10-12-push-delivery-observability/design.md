# Design

## El episodio que lo motivó

El change `webpush-notifications` se verificó en producción el **2026-10-12** con dos dispositivos
suscritos (Android y Brave) y un recordatorio vencido. El ciclo dejó esta línea:

```
reminder_push: ciclo completado delivered=1 skipped_old=0 pruned=0
```

Cuando el usuario preguntó si había llegado a **los dos** dispositivos, esa línea no permitía
responder. `delivered` es un contador de **recordatorios**, no de suscripciones: con dos
suscripciones, `delivered=1` significa «al menos una aceptó», y no distingue «las dos» de «una
aceptó y la otra respondió un 5xx». Para saberlo hubo que bajar a los `WARN` por endpoint del ciclo.
El hueco no lo encontró una revisión: **lo encontró el propio intento de verificar**. Eso es lo que
este change cierra —que el resumen del ciclo, por sí solo, responda la pregunta.

## Qué cuenta cada contador (y por qué `Gone` no es `failed`)

`TickOutcome` ya cuenta recordatorios (`delivered`, `skipped_old`, `pruned`, `unparseable`). Este
change añade tres contadores **por suscripción**, que miden el resultado de cada envío individual:

- `attempted` — envíos intentados en el ciclo.
- `accepted` — envíos que el push service aceptó (`Delivered`).
- `failed` — todo lo que no fue aceptado ni depurado: los rechazos del push service (`Failed`, cualquier código que no sea 404/410) y los errores de transporte (`Err(PushError)`: red, timeout, respuesta ilegible).

Una suscripción depurada por `Gone` (404/410) cuenta como **intentada** y como **depurada**
(`pruned`), **no** como fallida. La razón es que `Gone` no es un error: es la respuesta correcta y
esperada a «esta suscripción ya no existe», y el worker hace lo que debe —borrarla—. Meterla en
`failed` mezclaría dos diagnósticos distintos: «este endpoint rechazó el aviso» (algo que hay que
investigar) y «esta suscripción había caducado y se ha limpiado» (el sistema funcionando). Con
`delivered`/`accepted`/`failed`/`pruned` separados, un `failed=0 pruned=1` con `accepted=1` cuenta
la historia completa sin abrir un log.

## Por qué el predicado y no un `if` en el worker

La decisión «¿registro el resumen?» vive hoy como un `if` en línea dentro de `tick()`:
`if outcome.delivered > 0 || outcome.skipped_old > 0 || outcome.pruned > 0`. Un `if` embebido en
el bucle no se puede probar: para ejercitarlo haría falta **capturar el log**, y el repo no tiene
arnés para eso (no hay `tracing-test` ni subscriber de test). Sacarlo a un predicado puro
`should_log_summary(&self) -> bool` sobre los contadores hace que la decisión sea **verificable con
un `assert`** —un `TickOutcome` de prueba y su resultado booleano— sin depender del log. Por eso el
predicado es verdadero cuando **cualquiera** de los contadores difiere de cero: incluye los dos
casos que hoy se pierden, `unparseable` y `failed`.

## Lo que el predicado implica

El predicado tiene una consecuencia que conviene dejar dicha: como `attempted` es distinto de cero
**en cuanto hay un solo envío**, cualquier ciclo con trabajo —aunque todo salga bien— deja resumen.
El silencio queda reservado a los ciclos en los que **de verdad** no pasó nada: sin recordatorios
vencidos y sin envíos intentados. Esto es lo que se quiere. El resumen es el **recuento del trabajo
hecho**, no un aviso de incidencia: un ciclo sano con `attempted=2 accepted=2 failed=0` es
información, no ruido, y el silencio solo debe significar «no había nada que hacer».

## Por qué el endpoint se redacta (URL de capacidad) y por qué el host basta

El endpoint de una suscripción WebPush es una **URL de capacidad**: no es una dirección pública,
es un secreto que autoriza a enviar avisos a ese dispositivo concreto. Quien la tenga, puede
avisar a ese móvil. El proyecto ya trata las claves y los tokens como material que no va a los
logs —VAPID en `SENSITIVE_KEYS`—, y el endpoint pertenece a esa misma familia. Los `WARN` por
endpoint lo volcaban entero (`endpoint = %sub.endpoint`).

Para diagnosticar, el **host** basta: distingue un fallo de FCM de uno de Mozilla o de WNS, que es
la información que un operador necesita para orientarse. La ruta y el token de capacidad no aportan
nada al diagnóstico y sí filtran el secreto. Por eso los registros SHALL llevar solo el host y el
identificador de la suscripción, y la extracción del host SHALL ser un ayudante puro —probable sin
log y sin red—, con un marcador (`(endpoint no parseable)`) para los valores que no rinden host, de
modo que el fallo del propio ayudante nunca imprima la cadena original.

## Residuales, sin maquillar

1. **Los contadores no dicen *qué* endpoint falló más allá del host.** Con tres endpoints en el
   mismo host, `failed=1` solo señala el host. El identificador de la suscripción en el registro
   desambigua, pero el resumen agregado, por diseño, no.
2. **Un fallo parcial sigue marcando el recordatorio como entregado.** Si una suscripción acepta y
   otra falla, `delivered=1` y el aviso se consume: el dispositivo que falló no lo recibe y ya no
   se reintenta. Es la semántica *at-least-once* actual y **este change no la cambia**; arreglarlo
   es una decisión de entrega aparte, no de observabilidad.
3. **El contenido exacto de la línea de log no se asevera en tests.** No hay arnés de captura de
   `tracing` en el repo, y añadirlo sería otro change. La redacción del endpoint y la forma del
   resumen se verifican **leyendo el código**; los tests cubren los contadores, el predicado y el
   ayudante de host, que es lo que se puede probar sin capturar logs. Queda dicho aquí para que
   nadie confunda «no se prueba» con «no importa».
