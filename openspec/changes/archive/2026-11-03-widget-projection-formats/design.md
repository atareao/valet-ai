# Design — widget-projection-formats

## Contexto

El change `widget-projection` construyó un proyector determinista y puro: al cierre del turno mira el
texto final del asistente y, si reconoce una forma de presentación, sintetiza el widget sin depender de
que el modelo llame a `render_widget`. El proyector se desplegó como v0.26.0 y la verificación end-to-end
midió un **falso negativo** sobre un turno real: una lista numerada de alternativas
(`1. **Cine…**`, `2. **Cena…**`, `3. **Tarde de gimnasio…**`) cerrada por la interrogativa «¿Cuál
prefiere?» no produjo ningún widget.

El problema no es que el detector sea demasiado estricto en abstracto, sino que **no reconoce los formatos
que el modelo emite de verdad**. La causa raíz concreta está en `question_options_block`: exige que la
interrogativa de cierre esté en la línea **inmediatamente** siguiente al último ítem de la lista. En el
texto real hay una **línea en blanco** entre el último ítem y la interrogativa, de modo que la
comprobación recae sobre la línea vacía y la función devuelve `None`. A esto se suman dos formatos que el
prompt base de producción favorece y que ningún detector cubre hoy: el texto de las opciones conserva los
marcadores de énfasis Markdown (`**…**`) y las alternativas presentadas como **tabla Markdown** no las
reconoce nadie. Este change extiende el detector para cubrir esos tres huecos sin cambiar ni el esquema,
ni la forma del payload del widget, ni el frontend.

## Decisiones

### D1 — Tolerar líneas en blanco entre la lista y la interrogativa

`question_options_block` SHALL saltar las líneas **solo en blanco** (sin contenido) que haya entre el
último ítem de la lista y la interrogativa de cierre, tomando como interrogativa la primera línea **no
vacía** posterior. Es el formato real del modelo: separa visualmente la lista de su pregunta de cierre.
No se tolera texto intermedio: si entre la lista y la interrogativa hay una línea con contenido que no es
la pregunta, la detección sigue fallando, porque entonces ya no es un cierre de la lista.

### D2 — Ignorar el énfasis Markdown al extraer el texto, sin tocar el `span`

La extracción de etiquetas, opciones e ítems SHALL ignorar los marcadores de énfasis Markdown
(`**…**`, `__…__`, `*…*`, `_…_`, `` `…` ``) y conservar el texto interior, de modo que el widget muestre
`Cine…` y no `**Cine…**`. El `span` SHALL calcularse siempre sobre el texto **original**: los marcadores
forman parte del contenido persistido y el `span` debe seguir delimitando exactamente el tramo original
que el frontend compone. Recortar el `span` para excluir los marcadores dejaría de corresponder al texto
persistido y rompería la composición por tramos.

### D3 — Detector de tabla de opciones, con precedencia tras la de checklist

Un detector nuevo SHALL reconocer una **tabla Markdown de alternativas** como un widget de opciones cuando
se cumpla una de estas condiciones: las filas llevan etiquetas **consecutivas** (`A`, `B`, `C` / `1`, `2`,
`3`) o la **cabecera** nombra opciones (`Opción`/`Opcion`/`Alternativa`/`Option`). La detección de tabla
de **checklist** (primera columna con índice o casilla) SHALL conservar la **precedencia**: una tabla de
tareas no SHALL degradarse a opciones. El orden de evaluación del detector SHALL ser: tabla de checklist
antes que tabla de opciones.

### D4 — El `span` no cambia de semántica

Ni D1 ni D2 ni D3 alteran la semántica del `span`: sigue siendo un rango **semiabierto** `[start, end)`
en **bytes UTF-8** sobre el contenido original que corresponde exactamente al bloque reconocido. En D1 el
`span` delimita SOLO el bloque de la lista, sin incluir las líneas en blanco intercaladas ni la
interrogativa de cierre; en D3 delimita la tabla. No hay migración: el campo ya existe.

## Alternativas descartadas

- **Cambiar el prompt base para que el modelo escriba la interrogativa pegada a la lista.** Descartado: el
  modelo imita su propio precedente y seguirá emitiendo el formato que ya usa; además el prompt de
  producción *ordena* listas y tablas. Ajustar el detector es más robusto y determinista que confiar en
  que el modelo cambie su estilo.
- **Exigir que la interrogativa vaya pegada a la lista (status quo).** Descartado: es justamente el
  formato real del modelo el que tiene la línea en blanco; mantener la exigencia perpetúa el falso
  negativo medido.
- **Detectar cualquier tabla Markdown de dos columnas como opciones.** Descartado por el exceso de falsos
  positivos: hay tablas de datos que no son alternativas. Por eso se exige etiquetas consecutivas o una
  cabecera que nombre opciones.

## Riesgos

El riesgo principal es el **falso positivo**: una tabla genérica de dos columnas podría confundirse con
una lista de opciones y proyectar un widget que el usuario no pidió. La mitigación es precisa y está en
D3: no basta con que sea una tabla, SHALL cumplir etiquetas consecutivas o cabecera que nombre opciones.
El cambio se mantiene **puro y sin I/O**, y cada nueva detección —y cada nueva **no** detección— queda
cubierta por tests unitarios deterministas.

## Preguntas abiertas

Si en la verificación aparecen otros formatos reales que hoy no se cubren —por ejemplo listas **sin**
interrogativa de cierre pero con una instrucción del tipo «elige una»—, se evaluarán aparte, en un change
propio, con su propia evidencia medida. Este change se limita a los tres huecos verificados.

## Refinamientos tras auditoría (2026-11-02)

- **Énfasis equilibrado**: `strip_markdown_emphasis` empareja *runs* de delimitadores con reglas de *flanking*; preserva `snake_case`, `aarch64_linux`, `2 * 3` y marcadores desbalanceados. Fuzz adversarial de 200 000 cadenas: sin panics.
- **Cabecera de tabla**: la rama de etiquetas descarta la fila de cabecera (cuando `rows[1]` es separadora y `rows[0]` no es ya fila-etiqueta); una cabecera con primera celda índice (`#`) o casilla → se descarta como tabla de opciones (no degrada tareas).
- **`span`**: extraído `table_span(rows)`, compartido por checklist y opciones; semántica sin cambios.
- **Limitaciones deliberadas**: `option_label` solo acepta etiquetas de un dígito; el énfasis `*` intrapalabra no se recorta (se privilegia no romper `2*3`).
