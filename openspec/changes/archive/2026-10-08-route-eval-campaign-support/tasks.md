# Tasks

## 1. RED — tests que fallan (grupo 1: diagnóstico del fallo)

- [x] 1.1 Test: un turno no cubierto publica, por cada herramienta faltante, la skill que la habría cubierto, la **probabilidad** que el clasificador le dio y el **umbral efectivo** aplicado.
- [x] 1.2 Test: la **proximidad al umbral** se agrega en bandas y cuenta los near-miss con la distancia correcta.
- [x] 1.3 Test: un turno no cubierto publica la **fuente** de la selección (`Router` / `Error` / …) además de las probabilidades.

## 2. RED — tests que fallan (grupo 2: overrides desde fichero)

- [x] 2.1 Test: un fichero de overrides válido cambia el umbral **global** y los **por skill** y las **criteria** de las peticiones, **sin escribir** en `settings`.
- [x] 2.2 Test: la precedencia es `CLI > fichero > settings` (un flag `--threshold` pisa el fichero; el fichero pisa `settings`).
- [x] 2.3 Test: un fichero declarado y ausente o ilegible **falla ruidosamente** y no mide en silencio la configuración equivocada.
- [x] 2.4 Test: el informe declara qué overrides estaban activos y de dónde vino cada valor.

## 3. GREEN — implementación

- [x] 3.1 Diagnóstico: recoger `selection.probabilities` y `selection.source` y publicar, por tool faltante, la skill que la cubre, su probabilidad, el umbral efectivo y la fuente.
- [x] 3.2 Agregado de proximidad al umbral en bandas (conteo de near-miss).
- [x] 3.3 `--overrides <ruta>`: parseo JSON (umbrales global + por skill; criterios por skill) y fusión con la precedencia CLI > fichero > settings; `--dry-run` declara los overrides que aplicaría.

## 4. REFACTOR

- [x] 4.1 `cargo fmt` y `cargo clippy --all-targets -- -D warnings` sin warnings.
- [x] 4.2 Sin código muerto: reutilizar la resolución de umbral efectivo existente en vez de duplicarla.

## 5. Verificación

- [x] 5.1 `just check-all` en verde.
- [x] 5.2 `--dry-run` con un fichero de overrides declara la configuración y la ventana que usaría, sin red.
- [x] 5.3 Ejecución real de los 66 turnos: publicar el diagnóstico de los tres turnos no cubiertos (probabilidad, umbral, fuente) y confirmar que dos ficheros de overrides distintos dan dos mediciones distintas **sin tocar la BD**.

## 6. Cierre

- [x] 6.1 Review con `@rust-reviewer` (solo lectura) y hallazgos aplicados.
- [x] 6.2 PR a `development` y `openspec archive route-eval-campaign-support`.
