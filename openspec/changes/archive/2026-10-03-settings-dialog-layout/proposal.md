# Change: Pestaña «Generación» con sub-pestañas y diálogo de ajustes más ancho

## Why

Dos problemas de usabilidad detectados al estrenar la pestaña «Generación» y la de «Memoria
persistente»:

1. La pestaña «Generación» apila los cuatro roles (Chat, Colapso, Fichas y Consolidación) en
   vertical, con `<h4>` como separador. Con doce campos seguidos, la pestaña es una columna larga
   y difícil de recorrer. La pestaña «Prompts» ya resuelve lo mismo con sub-pestañas anidadas.
2. El diálogo de ajustes declara `width={600}` y monta **siete** pestañas. antd colapsa las que no
   caben en un desplegable de desbordamiento («…»), de modo que «Memoria persistente» no se ve ni
   se alcanza. Hace falta más ancho.

## What Changes

- **Pestaña «Generación» con sub-pestañas.** Dentro de «Generación» se monta un `Tabs` anidado con
  una sub-pestaña por rol —**Chat**, **Colapso**, **Fichas** y **Consolidación**— replicando el
  patrón de la pestaña «Prompts». Cada sub-pestaña contiene los tres campos del rol (temperatura,
  razonamiento, tokens máximos) con `name`/`label` iguales a su clave cruda; el bloque de
  consolidación usa `GENERATION_SEMANTIC_*`. Las sub-pestañas usan `forceRender` para que los doce
  campos sigan registrados en el formulario y el guardado (las doce claves) no cambie.
- **Diálogo de ajustes más ancho.** El `Modal` de ajustes pasa de `width={600}` a un ancho de al
  menos **860 px** (se fija en 900) para que las siete pestañas quepan en una fila y ninguna caiga
  en el desplegable de desbordamiento.

## Impact

- Afecta: `frontend/src/components/SettingsDialog.tsx`, `frontend/src/test/SettingsDialog.test.tsx`.
- No cambia contratos de datos ni API: se reutilizan `GET/PUT /settings` y las mismas doce claves
  `GENERATION_*`.
- No toca backend ni `docker-compose.prod.yml`.

### Fuera de alcance

- Reordenar o renombrar las pestañas superiores.
- Cambiar los rangos o validación de los campos de generación.
- Hacer el diálogo responsive más allá del ancho fijo indicado.
