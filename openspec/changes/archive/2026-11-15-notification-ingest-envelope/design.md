# Design

## `title` como fuente del mensaje, sin tocar el contrato actual

OpenObserve (alertas programadas) no emite `message`. La opción menos invasiva es aceptar `title`
como fuente alternativa del mensaje, en orden `message` → `title`. El contrato anterior (cuerpo con
`message`) no cambia: un `message` presente sigue ganando. Alternativas descartadas: un endpoint
nuevo (`/api/notifications/envelope`) —duplica autenticación y ruta—; un adaptador externo que
reescriba el cuerpo —una pieza de infra más que mantener para un único campo—; y exigir al productor
que reformatee —imposible, el webhook de OpenObserve es fijo—.

## `severity` como fuente del nivel, tolerante a lo desconocido

OpenObserve usa `severity` con `critical`/`warning`/`info`, que encajan 1:1 con la lista blanca de
`level`. Se consulta `severity` **solo** si falta `level` (`level` explícito manda). La diferencia de
trato es deliberada: un `level` presente y fuera de la lista blanca sigue siendo `400` (contrato
propio), pero un `severity` desconocido **cae a `info`** en vez de rechazar: los valores de severidad
los dicta un productor externo que puede ampliarlos sin avisar, y romper la ingesta por eso sería
peor que un nivel conservador.

## `body_markdown` en `metadata`, sin pisar

El `message` derivado de `title` es corto (`[critical] traefik_high_403_429 on traefik_access`) y no
lleva el detalle del disparo, que vive en `body_markdown`. Se anexa a `metadata["body_markdown"]`
para no perderlo, sin pisar una clave homónima que el productor ya hubiera enviado, y contando para el
límite de 8192 bytes ya existente. Alternativas descartadas: concatenarlo al mensaje —mezcla dos
formatos y acerca el mensaje a su límite de 4096—; guardar el envelope entero —bloat y duplicación
del esquema de OpenObserve—.

## El resto del envelope se ignora

`version`, `fields`, `rows`, `links`, `context` y `chart_url` no se interpretan. El struct ya tolera
campos desconocidos (serde los ignora), así que no se toca nada para ignorarlos; documentarlos como
no-goal fija la intención.
