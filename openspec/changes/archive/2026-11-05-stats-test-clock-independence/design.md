# Design: El test de la ventana temporal deja de depender del reloj

## Context

Ver `proposal.md` — Why. El test `db::repos::stats::tests::test_background_summary_last_call_at` mezcla dos
cosas incompatibles: un **fixture con fecha absoluta** y una **consulta con ventana relativa**
(`created_at >= datetime('now', '-' || ?1 || ' days')`). Por eso el test es una bomba de reloj: no falla por un
cambio de código, falla porque pasa el tiempo.

La causa raíz, con detalle, es doble:

1. **Comparación de cadenas.** `created_at` es `TEXT` y SQLite ordena/compara como texto. El corte que
   produce `datetime('now', '-30 days')` tiene el formato `YYYY-MM-DD HH:MM:SS` (con **espacio** en la
   posición 10), mientras que el fixture usaba `YYYY-MM-DDTHH:MM:SS` (con `T` en la posición 10). Como
   `'T'` (0x54) > `' '` (0x20), el fixture del 5 de octubre pasaba por delante del corte del 4 de octubre y
   entraba en la ventana. Es decir: el test pasaba **por accidente** del formato, no por la semántica.
2. **El corte se desplaza cada día.** Al cambiar el día (2026-11-05), el corte pasó a `'2026-10-06 …'`, que ya
   es mayor que el fixture en comparación de cadenas, y la fila quedó excluida.

Ninguno de los dos problemas está en la consulta, que es correcta y es la que corre en producción.

## Goals / Non-Goals

**Goals:**
- El test pasa **cualquier día**, sin depender de la fecha del calendario ni de la zona horaria del proceso.
- Cubre de forma explícita y determinista la pertenencia a la ventana (dentro / fuera / sin filas).
- `cargo test`, `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` en verde.
- Desbloquea el CI de `main` y `development`.

**Non-Goals:**
- No cambiar la consulta de `background_summary` ni su firma ni el significado de sus parámetros.
- No introducir inyección de reloj, ni `chrono` en los tests, ni una dependencia de congelación de tiempo.
- No tocar los otros tests que usan fechas absolutas antiguas: `src/db/repos/notifications.rs`
  (`'2020-01-01 …'`) y `src/db/repos/push.rs` (`'2000-01-01 …'`) las usan solo para **forzar el orden** de las
  filas, sin ventana relativa, y `src/tools/unified_search.rs` (`'2025-01-01T…Z'`) las usa como valor estático
  de aserción. No son bombas de reloj y quedan fuera del alcance.
- No se persigue un detector genérico de «tests con fecha fija».
- Los fixtures con fecha absoluta que quedan en `stats.rs` fuera de los tests de ventana
  (`test_export_csv`, `test_export_csv_includes_kind_column`, `test_db_sizes_includes_timeline_events`) se
  dejan como están: ninguno tiene ventana temporal (`export_csv` ordena por `created_at` sin filtrar y
  `db_sizes` solo cuenta filas), así que sus fechas no caducan.

## Decisions

### D1. Fechas relativas con el reloj de SQLite, no con el de Rust

Los fixtures se insertan con el helper existente y después se ajustan con
`UPDATE llm_requests SET created_at = datetime('now', '-N days') WHERE id = ?1`. Se usa el reloj de **SQLite**
porque es exactamente el que evalúa la ventana: así el test y la consulta comparan contra el mismo origen de
tiempo, con el mismo formato (`YYYY-MM-DD HH:MM:SS`) y sin depender del formato que produjese Rust.

- Alternativa descartada: calcular la fecha en Rust con `chrono` y pasarla como literal. Reproduce el
  problema del formato (habría que acertar el separador y la zona) y añade una dependencia al test.
- Alternativa descartada: congelar el tiempo (`faketime`, crate de freezegun). Añade una dependencia y una
  variable de entorno al CI para un solo test; no hace falta.

### D2. La expectativa se lee de la base de datos, no se escribe a mano

`last_call_at` se compara contra el valor que devuelve un `SELECT created_at FROM llm_requests WHERE id = 'lc-c'`,
no contra un literal. La invariante que se comprueba es «es la fecha de la fila más reciente **dentro de la
ventana**», no «es esta cadena concreta».

### D3. La pertenencia a la ventana se comprueba cambiando `days`, no la fecha

El mismo juego de datos se consulta dos veces:

- con `days = 30`: la fila de `-40 days` queda fuera → `calls` cuenta solo las de dentro;
- con `days = 90`: la fila de `-40 days` queda dentro → `calls` sube.

Así se verifica que la ventana se evalúa **relativa a ahora** sin escribir ni una fecha absoluta, y el test
sigue siendo válido dentro de diez años.

### D4. Se conserva la aserción negativa del origen sin filas

`archivist` no tiene filas y su `last_call_at` debe ser `None`. Es la otra mitad de la semántica de
`last_call_at` y se mantiene.

### D5. Juego de datos propuesto

Tres filas de `kind = 'router'`, `model = 'A'`:

| id     | `created_at`                | Papel                                        |
|:-------|:----------------------------|:---------------------------------------------|
| `lc-a` | `datetime('now','-40 days')`| Fuera de la ventana de 30 días                |
| `lc-b` | `datetime('now','-2 days')` | Dentro de la ventana; no es la más reciente   |
| `lc-c` | `datetime('now','-1 day')`  | Dentro de la ventana; **es** la más reciente  |

Se mantienen los offsets en **días enteros distintos** a propósito: `datetime('now')` tiene granularidad de
segundo y dos filas escritas en el mismo segundo empatarían, haciendo el test dependiente del orden.

### D6. Se homogeneizan los tres fixtures hermanos del mismo fichero

`test_background_summary_days_window`, `test_by_model_days_window` y `test_purge_old` asertan **exclusión**
sobre una fecha absoluta: hoy son estables (la fecha solo envejece), pero sus comentarios («60 days ago»)
mienten y el fichero quedaba con dos idiomas para lo mismo. Se convierten a `datetime('now','-60 days')` con
el mismo helper, **sin cambiar ninguna aserción**. Se dejan fuera, en cambio, los tests del mismo fichero que
no tienen ventana temporal (ver Non-Goals) y los de otros ficheros.

## Risks / Trade-offs

- **Granularidad de segundo del reloj de SQLite.** Mitigado con offsets de días distintos (D5). No se usan
  offsets en segundos ni horas.
- **El test pasa a depender de que `(now - 1 day)` siga dentro de una ventana de 30 días.** Es intrínseco al
  caso y no puede romperse salvo que alguien cambie el parámetro `days` de la llamada, cosa que el propio
  test declara.
- **Quedan tests con fechas absolutas** (notifications, push, unified_search). Se documentan como
  deliberadamente fuera de alcance en los Non-Goals; ninguno tiene ventana relativa.
- **`background_summary` agrupa por `(kind, model)`.** Mantener las tres filas con el mismo par evita
  sorpresas de agrupación en las aserciones sobre `calls`.
- **`src/handlers/stats.rs` conserva fixtures absolutos en tests de ventana**
  (`test_by_model_handler_propagates_days`, `test_background_handler_propagates_days`, con
  `'2026-07-01T00:00:00'` sobre una ventana de 7 días). Hoy son seguros porque asertan **exclusión** y esa
  fecha solo envejece, pero son el mismo antipatrón. Quedan fuera de alcance y se registran aquí como riesgo
  residual.

## Migration Plan

No aplica: no hay migración de datos, ni despliegue, ni cambio de esquema. El cambio viaja como un PR a
`development`; al llegar a `main` deja el CI verde y permite retomar la cola de Dependabot.

## Open Questions

Ninguna.
