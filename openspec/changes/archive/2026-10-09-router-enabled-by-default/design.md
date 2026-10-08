# Design

## D1 — Por qué encender ahora

El enrutador se entregó apagado a propósito, para poder ajustarlo sin exponer a nadie. Ese trabajo terminó: el catálogo es cerrado, los umbrales son los medidos, las criteria del briefing reconocen el saludo de apertura y el arnés mide **100 % de cobertura con un 25,6 % de ahorro** (override A/B, ambos lados con `--repeat 3`, 66 turnos de base real). El enrutador falla abierto —ante cualquier fallo expone todas las herramientas—, así que el peor caso del encendido es «como antes, con 320 ms más». Mantenerlo apagado ya no protege de nada y deja el ahorro sin cobrar.

## D2 — Por qué el valor sembrado es `true` y el default compilado sigue `false`

Son dos «defaults» distintos, con trabajos distintos:

- el **sembrado** (lo que deja la migración en una base migrada) → `true`: el enrutador arranca encendido, que es la decisión de producto;
- el **compilado** (`SkillRouterConfig::default()`), que solo entra cuando la clave está **ausente o ilegible** → `false`: sin la clave, el enrutador no manda nada al clasificador externo.

Poner también el compilado en `true` habría sido más «coherente» y más arriesgado: una clave ausente empezaría a enviar los mensajes del usuario a OpenRouter sin que nadie lo haya pedido. Se prefiere que la ausencia de configuración signifique «no salir a la red». Efecto colateral bueno: el test `read_router_config_falls_back_to_defaults_when_absent` sigue vigente y el change no toca `skill_router.rs`.

## D3 — La migración, y su compromiso honesto

`20261008000002_router_enabled_by_default.sql` replica el idioma de `20261008000001_skill_router_tuning.sql`: idempotente y ajustando **solo el valor exactamente sembrado** (`WHERE key = 'ROUTER_ENABLED' AND value = 'false'`), de modo que un valor distinto del usuario nunca se pisa.

El compromiso se documenta en el propio fichero: un usuario que hubiera vuelto a `false` **partiendo del sembrado** es indistinguible del sembrado y verá su elección revertida **una vez**. Es el mismo compromiso que la migración de umbrales asumió con `0.3→0.10`, y el riesgo práctico es bajo porque el enrutador nunca ha estado encendido en producción.

## D4 — Por qué el delta usa RENAMED y por qué el escenario «apagado» no se renombra

El header del requisito cambia («arrancar **apagada**» → «arrancar **encendida**»). Un `MODIFIED` debe referenciar un header que exista en la spec destino, así que el cambio de nombre se declara con `## RENAMED Requirements` (FROM/TO) y el bloque se reescribe bajo el nombre nuevo. Verificado en una copia de ensayo: `openspec archive` reporta `~ 1 modified` y `→ 1 renamed` y el header queda «encendida».

El escenario «Por defecto el enrutado está apagado» **no se renombra**: OpenSpec rechaza un `MODIFIED` que omita un escenario existente (renombrarlo cuenta como omisión) y no soporta renombrar escenarios. Se **repurposa** a lo que sigue siendo verdad —una `settings` **sin** la clave deja el enrutador apagado— y se **añade** «Por defecto el enrutado está encendido» para la base recién migrada. Los dos escenarios no se contradicen: describen los dos defaults de D2 (sembrado encendido, ausencia apagada).

## D5 — Qué NO cambia y qué NO promete

- No cambia el comportamiento en el mismo instante de desplegar el código: es la migración, al arrancar, la que fija el valor.
- No toca el default compilado, ni los umbrales, ni el catálogo, ni los criterios, ni la API, ni la UI.
- No arregla el arranque del contenedor tras un reinicio del host (Bloque D): si el host se apaga, el servicio no vuelve solo. Es otro change, ya identificado como aplazado.
- No promete la cobertura medida: el 100 % / 25,6 % es la evidencia de la decisión, medida sobre 66 turnos históricos, no una garantía sobre turnos futuros.

## D6 — El test que asumía el default

Cambiar un default obliga a revisar quién lo asumía. El único test que lo hacía es `router_disabled_exposes_all_tools` (`src/orchestrator/agent.rs`), cuyo comentario decía «`ROUTER_ENABLED` defaults to `false` from the migration». Deja de apoyarse en el default y **fija el estado** (`ROUTER_ENABLED=false`), igual que su test gemelo fija `true` para el caso encendido. La lección: un default no es un estado; un test que quiere «apagado» debe apagarlo. El resto de la suite no asumía el default —los tests que necesitan el enrutador encendido ya lo encienden explícitamente—.
