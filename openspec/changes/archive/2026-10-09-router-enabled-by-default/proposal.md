# Proposal

## Why

El enrutador lleva apagado desde su primer día (`ROUTER_ENABLED=false`): era lo correcto mientras se ajustaba, ya no. La campaña de medición lo dejó decidido: con el catálogo cerrado de seis dominios, los umbrales ajustados y las criteria del briefing matutino, el enrutador cubre el **100 %** de los turnos históricos medidos con `--repeat 3` y ahorra el **25,6 %** de las definiciones de herramientas por turno, fallando siempre abierto. Dejarlo apagado es renunciar al ahorro que justificó todo el trabajo.

El coste está medido: ~320 ms de latencia p50 añadida por turno y ~$0,000053 por turno de clasificador.

## What Changes

- Una migración SHALL sembrar `ROUTER_ENABLED=true`, de modo que una base recién migrada —y las existentes, si el valor sigue siendo el sembrado `false`— arranquen con el enrutador **encendido**. Idempotente y sin pisar una elección distinta del usuario.
- El **default compilado SHALL seguir siendo `false`**: si la clave desaparece o es ilegible, el enrutador queda apagado y no se envía nada al clasificador. Es la red de seguridad ante una `settings` restaurada a medias.
- El requisito «La configuración del enrutador SHALL vivir en settings, leerse en cada turno y arrancar apagada» SHALL pasar a «…arrancar encendida», conservando sus escenarios: el de arranque por defecto se mantiene para el caso de clave **ausente** y se añade el del arranque **encendido** tras migrar.
- **No** cambia nada más: ni umbrales, ni catálogo, ni herramientas, ni criterios, ni API, ni UI. El usuario sigue pudiendo apagarlo desde los ajustes.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `orchestrator/skill-router`: el valor sembrado de `ROUTER_ENABLED` pasa a `true` y el default compilado sigue siendo `false` (red de seguridad ante clave ausente).

## Impact

- Nueva migración `migrations/20261008000002_router_enabled_by_default.sql`.
- `tests/db/migrations.rs`: el valor sembrado esperado de `ROUTER_ENABLED` pasa de `false` a `true`.
- La spec `openspec/specs/orchestrator/skill-router/spec.md`.
- **No** toca `src/orchestrator/skill_router.rs`: el default compilado se queda en `false` y el test «absent defaults to false» sigue vigente.
- **Fuera de alcance**: el arranque del contenedor tras reiniciar el host (Bloque D: `restart: unless-stopped` + `podman-restart`) queda como su propio change.
- **Criterio de aceptación**: tras migrar, `ROUTER_ENABLED` vale `true` y el enrutador queda habilitado; sin la clave, queda apagado.
