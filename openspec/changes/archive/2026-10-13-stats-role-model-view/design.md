# Design

## Evidencia

Todo lo que sigue sale de la **base de datos de producción**, con las filas reales de `llm_requests`.
La consulta de contraste es:

```sql
SELECT kind, model,
       COUNT(*) AS calls,
       SUM(completion_tokens) / COUNT(*) AS avg_completion_tokens,
       SUM(CASE WHEN status != 'success' THEN 1 ELSE 0 END) AS errors
FROM llm_requests
WHERE kind != 'chat'
GROUP BY kind, model
ORDER BY kind, SUM(cost) DESC;
```

La fila que la UI muestra hoy para el consolidador es `89 llamadas · 242.522 tokens · 0 errores`, y
dentro de ella caben **tres modelos**:

| modelo | llamadas | tokens de salida de media | naturaleza |
|:---|--:|--:|:---|
| `deepseek/deepseek-v4.1-flash` | 54 | **1** | la avería del 9–10 de octubre (respuestas vacías con `status='success'`) |
| `qwen/qwen3-235b-a22b-2507` | 33 | 654 | el parche sano (`SEMANTIC_MODEL`) |
| `mistralai/mistral-small-24b-instruct-2501` | 2 | 356 | residual |

Además, el rol `timeline` con `mistral` acumula **7 errores en 20 llamadas (35 %)**.

De ahí salen los cuatro fallos que motivan este change:

1. la fila funde tres modelos en una cifra que no dice nada de ninguno;
2. `SEMANTIC_MODEL=qwen/...` **no aparece en ninguna parte de la UI**: ni en la tabla de roles (lo
   funde) ni en «Cost by Model» (que filtra `kind='chat'`, donde qwen no se usa);
3. la columna «Errors» cuenta `status != 'success'`, así que **no habría visto la avería**: las 54
   respuestas vacías son `success`;
4. la tabla suma **toda la vida**; un contador acumulado no responde «¿está funcionando ahora?».

---

## D1 — Un eje de modelo en `background_summary`

`BackgroundStats` gana `model: Option<String>` y la agregación pasa de `GROUP BY kind` a
`GROUP BY kind, model`. Los orígenes canónicos (`router`, `archivist`, `consolidator`, `collapse`,
`timeline`) **sin ninguna fila** siguen apareciendo con los contadores a cero y `model: null`: la
visibilidad de que un rol nunca corre es información, no ruido. En producción, `collapse` no tiene
ni una fila histórica; sin el relleno de ceros, desaparecería de la tabla y nadie sabría que no se
está ejecutando.

**Relleno de ceros sin perder las parejas con datos.** En SQL un `FULL OUTER JOIN` no existe en
SQLite; la forma de conservar *todas* las parejas reales **y** añadir una fila `null` por cada origen
canónico sin filas es resolverlo en Rust sobre el resultado agrupado: se recorre la lista de orígenes
canónicos en orden y, para cada uno, se emitían sus parejas (ordenadas por coste descendente) o, si
no tiene ninguna, una fila con `model: null` y contadores a cero. El orden final —por `kind` canónico
y, dentro, por coste— se fija en Rust, no en el `ORDER BY` de SQL, porque el orden canónico no es
alfabético.

**Alternativa descartada:** una sola fila «(sin modelo)» para los orígenes sin filas. Se descarta
porque mezclaría dos cosas distintas: un origen que nunca corre (`model: null` legítimo) y una pareja
que existe sin modelo registrado.

---

## D2 — `suspicious_calls`: por qué 1 token y por qué solo `success`

La condición es `status = 'success'` **y** `completion_tokens <= 1`, con el umbral en una constante
con nombre.

**Por qué el umbral es 1 y no un rango más amplio.** La avería medida son respuestas con
`completion_tokens = 1` exactamente y `content = null`. En el change archivado
`episodic-call-integrity` se reprodujo contra OpenRouter: DeepSeek, ante un único mensaje `system`,
devuelve `completion_tokens: 1`, `content: null`, `finish_reason: "stop"` (3/3). El umbral `<= 1`
caza ese caso exacto sin falsos positivos por respuestas cortas legítimas (un JSON válido del
extractor no baja de decenas de tokens). Un umbral «bajo pero no 1» (p. ej. «menos de 10 tokens»)
marcaría respuestas legítimamente cortas y convertiría la columna en ruido.

**Por qué solo `success`.** El objetivo es cazar el fallo **invisible**: el que el código y la
columna de errores no ven. Una llamada con `status='error'` y salida vacía **ya** cuenta en
`total_errors` y ya se ve; sumarla también a `suspicious_calls` duplicaría la señal y haría que las
dos columnas contaran la misma fila. `suspicious_calls` es específicamente «éxito con forma de
avería»: el `success` que no lo parece.

**Por qué solo orígenes estructurados.** Los orígenes canónicos devuelven JSON estructurado; una
salida de un token no puede ser un JSON válido del contrato. En `chat`, en cambio, una respuesta de
un token puede ser legítima («Sí»), así que la marca **no SHALL** aplicarse nunca a `chat`. Como la
propia consulta de fondo ya excluye `chat`, la restricción es una garantía del contrato, no del
`WHERE`.

---

## D3 — `last_call_at`

`MAX(created_at)` por pareja, `None` si no hay filas. `created_at` es el mismo campo que ordena la
retención y la purga, así que no requiere migración ni columna nueva: se agrega en la misma consulta
que ya agrupa. Es la respuesta directa a «¿está funcionando ahora?» que el acumulado no da.

---

## D4 — Ventana temporal, 7 días por defecto

`background_summary` acepta `days` y filtra `created_at >= datetime('now', '-' || ?1 || ' days')`.
El patrón del handler ya existe en `src/handlers/stats.rs` (`Query<DayParams>`,
`params.days.unwrap_or(...)`); se reutiliza sin inventar nada. El valor por defecto del handler es
**7**, no 30.

**Por qué 7 y no 30.** La función de esta ventana es responder «¿está funcionando ahora?». Con 30
días, la avería de dos días del 9–10 de octubre quedaría diluida entre tres semanas de llamadas
sanas; con 7, una racha de dos días de respuestas vacías domina la señal. El selector del dashboard
permite ampliar cuando se quiera mirar más atrás.

**Compatibilidad.** El parámetro es opcional: sin él la consulta sigue siendo válida y ningún
cliente actual se rompe. `days` SHALL propagarse a `by_model` para que las tres tarjetas del panel
(roles, modelos —y, por coherencia de ventana, la diaria—) miren el mismo tramo.

---

## D5 — Panel de modelos unificado

`StatsRepo::by_model` deja de filtrar `WHERE kind='chat'` y agrupa **por modelo, agregando todos los
orígenes**. Así un modelo que solo trabaja en roles de fondo (qwen) aparece con su coste total.

**Lo que NO se toca, a propósito.** El `WHERE kind = 'chat'` de `tools_summary` es correcto: las
herramientas solo se usan en conversación, y agregar las de fondo no tendría sentido. Lo mismo para
la consulta de `tool_calls`. El filtro por chat es **incorrecto** para `by_model` y **correcto** para
las herramientas; no se generaliza por simetría. `StatsRepo::summary` tampoco se toca (ver «Decisiones
abiertas»).

---

## Decisión consciente: el panel de modelos deja de ser solo de chat

El spec actual dice literalmente:

> Las tarjetas de chat (resumen, modelos y diaria) SHALL NOT incluir las peticiones de los orígenes
> de fondo.

Este change **revierte** esa restricción **para el panel de modelos**, y solo para él. No es un
descuido: es la consecuencia directa de la evidencia.

- **Qué se gana.** Un modelo configurado en producción deja de ser invisible. `SEMANTIC_MODEL=qwen/...`
  aparece con su coste total; hoy no aparece en ninguna tarjeta. La decisión de qué modelo usar para
  los roles de fondo —una decisión de coste, ya no de viabilidad tras `episodic-call-integrity`— pasa
  a apoyarse en un número visible.
- **Qué se pierde.** Los totales del panel de modelos **dejan de ser «solo conversación»**. Quien
  leyera «Cost by Model» como el coste del chat ahora verá un total mayor, con el gasto de fondo
  mezclado. Por eso el título y la etiqueta del panel SHALL cambiar para reflejarlo, y las tarjetas
  de resumen y diaria **siguen** sin incluir los orígenes de fondo.
- **Qué queda en pie.** La separación entre conversación y trabajo de fondo se conserva donde tiene
  sentido (resumen, diaria, herramientas); se rompe solo donde la fusión escondía justo lo que se
  quiere vigilar.

La tarjeta «Procesos de fondo» sigue existiendo y sigue siendo el desglose por rol y modelo; el panel
de modelos es la vista agregada por modelo. Son dos cortes del mismo dato, y ahora ambos son visibles.

---

## Decisiones abiertas

### (a) `StatsRepo::summary` sigue filtrando `WHERE kind='chat'` — los totales no cuadrarán

Añadidos los orígenes de fondo al panel de modelos (D5) pero no al resumen (que este change no toca),
**el total del panel de modelos ya no cuadrará con el total del resumen**. No es un bug introducido:
es una asimetría que ahora se vuelve visible porque los dos números vivirán cerca en la misma
pantalla.

- **Opción 1 — dejarlo como está.** El resumen sigue siendo «solo conversación»; la diferencia es
  esperada y se documenta. Coste: dos cifras de coste que no suman puede confundir y parecer un
  fallo.
- **Opción 2 — unificar `summary` también (retirar su `kind='chat'`).** Los dos totales cuadrarían.
  Coste: se pierde el «coste del chat» como cifra propia, que hoy es información útil y coherente con
  la diaria.
- **Opción 3 — exponer ambos totales con etiquetas explícitas** («chat» y «total incluido fondo»).

**Propuesta como follow-up:** no se resuelve en este change. Se pide decisión al usuario antes de
tocarlo, porque cambia el significado de la tarjeta de resumen y de la diaria, que hoy comparten el
filtro por chat con las herramientas.

### (b) `by_day` por defecto 30 frente a la nueva ventana por defecto 7

`StatsRepo::by_day` tiene hoy un default de **30** días en su contrato; la ventana nueva del panel
tiene default de **7**. Si el dashboard siempre pasa el valor seleccionado, el default del repositorio
nunca se usa desde la UI y la discrepancia es inocua. Se deja **a propósito** que el default del
repositorio (30) y el del dashboard (7) difieran, porque significan cosas distintas: 30 es el default
de la *serie histórica* cuando alguien llama a `by_day` sin argumentos (p. ej. desde una herramienta
o un test), y 7 es el default de la *ventana de vigilancia* del panel. Unificarlos acoplaría dos
decisiones independientes. El requisito del selector SHALL (`frontend`) fija que el dashboard **siempre
pasa** el valor seleccionado, así que nunca depende del default del repositorio.

---

## No-goals

- No se toca `tools_summary` ni la consulta de `tool_calls`: su filtro por chat es correcto.
- No se resuelve la asimetría de `summary` (decisión abierta (a)).
- No se unifica el default de `by_day` con el de la ventana (decisión abierta (b)).
- No se cambia la retención ni la purga.
- No se toca `docker-compose.prod.yml`.
