# Change: La proyección de widgets SHALL reconocer los formatos que el modelo emite de verdad

## Why

La proyección determinista del change `widget-projection` se desplegó como v0.26.0, pero la verificación
end-to-end midió un **falso negativo**: un turno real con una lista numerada de opciones
(`1. **Cine…**`, `2. **Cena…**`, `3. **Tarde de gimnasio…**`) cerrada por «¿Cuál prefiere?» no produjo
ningún widget.

Causa raíz verificada leyendo `src/orchestrator/widget_projection.rs`: la función `question_options_block`
exige la interrogativa de cierre en la línea **inmediatamente** siguiente al último ítem de la lista. En el
texto real hay una **línea en blanco** entre el último ítem y la interrogativa, así que la comprobación
cae sobre la línea vacía y la detección devuelve `None`. Además: (a) el texto de las opciones conserva los
marcadores de énfasis Markdown (`**…**`); (b) un riesgo ya conocido, las opciones presentadas como **tabla
Markdown** no las reconoce ningún detector. El prompt base de producción *ordena* listas y tablas, de modo
que estos son precisamente los formatos que el modelo emite.

## What Changes

1. `question_options_block` SHALL tolerar **líneas en blanco** (solo en blanco, sin contenido) entre la
   lista y la interrogativa de cierre, tomando como interrogativa la primera línea **no vacía** posterior.
   El `span` SHALL seguir delimitando SOLO el bloque de la lista, sin incluir las líneas en blanco ni la
   interrogativa.
2. La extracción de etiquetas, opciones e ítems SHALL ignorar los marcadores de **énfasis Markdown**
   (`**…**`, `__…__`, `*…*`, `_…_`, `` `…` ``), dejando el texto interior, y SHALL NOT alterar el `span`,
   que se calcula siempre sobre el texto original.
3. Un detector nuevo de **tabla de opciones** SHALL producir un `Options` cuando la tabla Markdown tenga
   filas de alternativas etiquetadas con etiquetas consecutivas (`A`, `B`, `C` / `1`, `2`, `3`) o una
   cabecera que nombre opciones (`Opción`/`Opcion`/`Alternativa`/`Option`). La detección de tabla de
   checklist (índice o casilla) SHALL conservar la precedencia para no degradar tareas a opciones.

## Impact

- Backend: `src/orchestrator/widget_projection.rs` (detector). Sin cambios de esquema, sin migración (el
  `span` ya existe). Sin cambios de frontend (la forma del payload del widget no cambia).
- Specs: `orchestrator/agent` (1 requisito MODIFICADO).
- Recoge la verificación 10.4/10.5 trasladada desde el change `widget-projection`.
