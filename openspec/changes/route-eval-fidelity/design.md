# Design

## D1 — Por qué la ventana por tokens y no la corrida contigua

Producción (`orchestrator::agent`) carga el historial con `MessagesRepo::list_by_token_budget(&db, max_window_tokens)`, donde `max_window_tokens` sale de `settings.max_window_tokens` con **default 10000**. Esa consulta devuelve los mensajes más recientes que caben en el presupuesto, contados **hacia atrás** (`SUM(...) OVER (ORDER BY created_at DESC ROWS UNBOUNDED PRECEDING)`), y en orden cronológico. Después el agente trunca cada entrada a 400 caracteres y el enrutador conserva las últimas `ROUTER_HISTORY_TURNS`.

Comprobado además (y necesario para la fidelidad): el mensaje del usuario **se persiste después** de cargar la ventana y de decidir, así que el historial del clasificador **no** lo incluye. La suposición del arnés —el historial son los mensajes estrictamente anteriores— es correcta y se mantiene.

Por tanto el arnés deja de inventar una corrida contigua y **reproduce la ventana**: para cada turno, cuenta hacia atrás desde su mensaje de usuario acumulando el coste en tokens efectivo (`collapsed_tokens_count ?? tokens_count`) hasta agotar el presupuesto. El reinicio en los cortes de emparejamiento desaparece: era una restricción que producción no tiene y que dejaba las continuaciones sin contexto.

## D2 — Cómo se mide el ahorro

Se serializan las **mismas definiciones que viajan en la petición** —`registry.definitions_for(&exposed)`— con `serde_json`, y se cuentan:

- **bytes**: longitud del JSON serializado;
- **tokens estimados**: `token_estimate::estimate_json_tokens` sobre ese mismo JSON, el estimador propio del proyecto.

El referente es el **conjunto completo habilitado** (`registry.definitions()`, el que viajaría sin enrutador). El informe publica la media por turno de herramientas expuestas, de bytes y de tokens estimados, y el ahorro porcentual frente al referente.

Se evita a propósito una tabla de tamaños por herramienta escrita a mano: envejecería en silencio al cambiar un esquema y convertiría la cifra de ahorro en una estimación. Midiendo lo que se serializa, la cifra es del informe.

## D3 — `--repeat` y la varianza

`--repeat <N>` repite el **barrido completo** N veces (N × turnos llamadas al clasificador; a ~$0,00003/turno, tres repeticiones de 66 turnos cuestan ~$0,006) y publica la cobertura de cada repetición y el **mínimo, la media y el máximo**.

El change anterior fijó un dintel de aceptación de «≥97 %» como **punto único** sobre una medida cuya varianza medida es de ±2-3 pp: un dintel no verificable con una sola muestra. Con `--repeat`, la decisión de encender se lee como rango. No se introduce ninguna noción de «aprobado» en el arnés: publica, no juzga.

## D4 — Qué NO cambia

- El recorte por turno a 400 caracteres y el recorte a `ROUTER_HISTORY_TURNS`: los aplica producción y el arnés ya los respeta. Alinearlos hacia otro lado sería alejarse.
- El catálogo, los umbrales, los criterios, la API y la UI: este change **no** altera comportamiento de producción. El enrutador sigue apagado.
- La semántica de la métrica de cobertura (un turno sin `tools_used` parseable nunca cuenta como cubierto) y el tratamiento de los turnos corruptos.

## D5 — Qué dirime esta alineación

Si tras alinear el arnés la cobertura se acerca al 97,0 % del experimento, la causa del hueco era el contexto y queda resuelto. Si **sigue** por debajo, entonces el hueco era del experimento —que también era una aproximación, con su propio historial construido a mano desde Python— y se documenta con las dos cifras delante, sin retocar ninguna a ojo. En ninguno de los dos casos se cambia un valor del catálogo para «hacer pasar» una cifra.
