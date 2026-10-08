# Proposal

## Why

El ajuste del enrutador se decidió con una cifra que **no es la que producción produciría**. La auditoría del change anterior confirmó que el arnés construye el estado del clasificador de tres formas que producción nunca usa:

1. Lee la columna `content` **cruda**, cuando producción usa `collapsed_content ?? content`.
2. Toma la **corrida contigua** de mensajes con un **reinicio artificial** en cada corte de emparejamiento, cuando producción usa la **ventana por presupuesto de tokens** (`max_window_tokens`).
3. Por ese reinicio, las **continuaciones** —la clase de fallo dominante— llegan al clasificador sin el contexto que sí tienen en producción.

Cinco ejecuciones dieron **92,4-95,5 %** donde el experimento predijo 97,0 %. La varianza de Jev (medida: ±2-3 pp) explica parte del hueco, pero no todo: la diferencia de contexto es **sistemática**. Mientras exista, **ninguna de las dos cifras es «la cobertura de producción»**, y la decisión de encender el enrutador se toma a ciegas.

Además el informe no publica **herramientas por turno** ni **ahorro**: el 28,1 % del diseño procede del análisis del experimento, no de una ejecución del arnés, así que la palanca que justifica la feature no es reproducible desde el propio arnés.

## What Changes

- **Fidelidad del estado del clasificador**: el arnés SHALL construir el historial exactamente como `orchestrator::agent` — `collapsed_content ?? content` por mensaje, la ventana fijada por `max_window_tokens` de `settings` (default 10000) contada hacia atrás desde el turno, y **sin reinicio** en los cortes de emparejamiento. El recorte por turno (400 caracteres) y el recorte a `ROUTER_HISTORY_TURNS` **no cambian** (son de producción y el arnés ya los respeta).
- **Instrumentación de la palanca**: el arnés SHALL publicar las **herramientas medias por turno** y los **bytes y tokens estimados** del bloque de definiciones expuesto frente al conjunto completo habilitado, con el **ahorro** resultante. Se mide serializando las **mismas definiciones que viajan en la petición** (`definitions_for`) y con el estimador del propio proyecto (`estimate_json_tokens`), no con una tabla de tamaños escrita a mano.
- **La varianza se publica**: `--repeat <N>` repite la medición N veces y publica la cobertura de cada repetición junto al mínimo, la media y el máximo, para que el dintel de encendido se lea como **rango** y no como el punto único que resultó estar mal calibrado.
- **BREAKING** (solo para el arnés): el `--dry-run` deja de declarar el emparejamiento en el sentido antiguo y pasa a declarar la **ventana** que habría usado.
- **Sin cambios** en el comportamiento de producción: ni el enrutador (`ROUTER_ENABLED=false`), ni el catálogo, ni los umbrales, ni las herramientas, ni la API, ni la UI.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `orchestrator/skill-router`: el requisito del arnés añade la **fidelidad con el contexto de producción**, la publicación de **herramientas por turno y ahorro**, y la **repetición con rango**.

## Impact

- `src/bin/route-eval.rs` (construcción del historial, instrumentación y `--repeat`) y sus tests.
- Depende de `settings.max_window_tokens` (ya existe; **no** requiere migración).
- **No** toca: el catálogo, el enrutador, `agent.rs`, la API, la UI, ni ninguna migración.
- **Criterio de aceptación de este change**: con el arnés alineado, la re-medición debe acercarse a la cifra que predijo el experimento o **explicar con datos el hueco que quede**; y el ahorro y las herramientas por turno deben ser cifras del informe, no del análisis.
