# Proposal

## Why

El `LocationWidget` ofrece hoy un botón «Guardar» cuya acción `onAction("save_place", …)` **no tiene ningún consumidor en el backend**: genera un turno de usuario que nadie interpreta —no hay tabla de lugares ni herramienta que los guarde, y el `design.md` archivado del widget ya lo declaraba «sin persistencia» y excluía la persistencia de lugares como non-goal—. En su lugar hace falta una acción que sirva de verdad: llevar la ubicación a la aplicación de mapas del dispositivo.

## What Changes

- **ELIMINADO — El botón «Guardar».** Desaparece `onAction("save_place", …)`. El widget deja de emitir acciones hacia el backend y pasa a ser presentacional.
- **NUEVA — La acción «Abrir»**, que lleva el punto a la aplicación de mapas del dispositivo, resolviendo el destino por plataforma: esquema `geo:` sin fijar paquete en Android —de modo que el sistema ofrezca el selector de aplicaciones o use la aplicación de mapas predeterminada del usuario—, URL de Apple Maps en iOS y la URL oficial de Google Maps en el resto.
- **MODIFICADO — El destino se deriva de las coordenadas**, nunca de la dirección ni del título. La etiqueta viaja solo como decoración codificada donde el esquema lo permite, jamás como consulta de búsqueda.
- **MODIFICADO — «Abrir» inicia la navegación en el contexto actual**, sin abrir una pestaña nueva: un esquema que el dispositivo no pueda atender no debe dejar una pestaña en blanco.
- **SIN CAMBIOS — «Cómo llegar» y «Copiar coordenadas»** se quedan como están.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `frontend`: el requisito «El widget LocationWidget SHALL mostrar una ubicación en un mapa con acciones» se sustituye por «El widget LocationWidget SHALL mostrar una ubicación en un mapa con las acciones Abrir, Cómo llegar y Copiar coordenadas» — cambian la lista de acciones que ofrece el widget y el contrato de la nueva acción «Abrir». El delta se expresa como `REMOVED` y `ADDED` en lugar de `MODIFIED` porque OpenSpec rechaza que un bloque `MODIFIED` elimine escenarios que la spec actual sí tiene.

## Impact

- Afecta: `frontend/src/components/widgets/LocationWidget.tsx`, un módulo nuevo de resolución de enlaces en `frontend/src/components/widgets/`, `frontend/src/test/LocationWidget.test.tsx`, `openspec/specs/frontend/spec.md` y la copia versionada de la build en `static/`.
- Sin cambios en Rust, sin migraciones, sin claves nuevas en `settings`. El tool `render_widget` y el esquema de su argumento `data` no se tocan: describen la forma de los datos, no las acciones del widget.
- Sin dependencias nuevas: la resolución de enlaces no añade librerías y el widget conserva su carga diferida.

## Non-goals

- NO se cambia «Cómo llegar»: sigue abriendo la búsqueda de Google Maps en una pestaña nueva. Queda anotado que, en escritorio y en Android, «Abrir» y «Cómo llegar» acaban en Google Maps con la misma consulta de coordenadas, y que convertir «Cómo llegar» en indicaciones de ruta reales es trabajo aparte.
- NO se añade un ajuste de proveedor de mapas preferido.
- NO se persisten lugares: sigue sin haber tabla de lugares.
- NO se toca `docker-compose.prod.yml`.
