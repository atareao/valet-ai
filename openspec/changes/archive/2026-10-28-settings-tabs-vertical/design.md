# Design

## Context

Ver `proposal.md` → *Why*. Lo que condiciona el diseño:

- Los seis `Tabs` del diálogo están repartidos en **tres** ficheros: cuatro en
  `SettingsDialog.tsx` (superior, «Prompts», «Memoria», «Generación»), uno en `SkillsTab.tsx` y uno
  en `IntegrationsTab.tsx`. Los dos últimos son componentes hermanos que el diálogo ya monta con
  `destroyOnHidden` por apertura.
- antd 5.29 (rc-tabs 8) aplica `aria-orientation="vertical"` al `role="tablist"` **solo** cuando la
  orientación es lateral (`node_modules/rc-tabs/lib/TabNavList/index.js:572`). Los tests pueden
  asertar la orientación por atributo accesible, sin mirar clases CSS ni `style`.
- `SettingsDialog.test.tsx` tiene hoy una aserción que exige justo lo contrario
  (`not.toHaveAttribute("aria-orientation", "vertical")`, línea ~2722) y un test que fija las diez
  pestañas superiores por nombre y orden (`shows exactly ten top-level tabs…`), que **no** cambia.
- Varios `Tabs` usan `forceRender` (Prompts, Generación, Integraciones) porque hay campos o
  indicadores que deben estar montados desde el primer render; «Memoria» no lo usa a propósito
  (montaje perezoso del panel persistente). La orientación no afecta a ese mecanismo.

## Goals / Non-Goals

**Goals:**

- Que **los seis** `Tabs` del diálogo de ajustes se rendericen con `tabPosition="left"`, con un
  cambio de una línea por `Tabs` y sin tocar nada más.
- Que la orientación sea verificable por tests de accesibilidad (`aria-orientation="vertical"`) y no
  por detalle de implementación.
- Dejar el spec sin contradicciones: ni «sub-pestañas horizontales» ni un requisito de
  «que quepan en una fila» que ya no aplica.

**Non-Goals:**

- `StatsDashboard.tsx` también usa `Tabs`, pero no pertenece al diálogo de ajustes: se queda
  horizontal.
- No se reordena ni se renombra ninguna pestaña, no se agrupan pestañas y no se añade navegación
  nueva (buscador, iconos, contadores).
- No se toca el ancho del `Modal` ni se introduce scroll nuevo.

## Decisions

### D1 — `tabPosition="left"` en todos los `Tabs`, sin punto de ruptura

**Decisión**: orientación lateral incondicional, en escritorio y en ventana estrecha.

**Por qué**: el usuario pide «todas las pestañas a verticales», y una orientación que cambia según
el ancho produce dos disposiciones que mantener y dos juegos de tests. Las etiquetas son cortas (la
más larga, «Enrutador de skills», ocupa una línea en ~170 px) y la app se usa mayoritariamente en
escritorio; el `Modal` ya está acotado al viewport por antd.

**Alternativas descartadas**:

- `useBreakpoint()` → `left` en `md+`, `top` en móvil. Añade un hook por componente, dos caminos de
  render y tests condicionados al ancho. Se puede introducir después sin romper nada si aparece una
  queja real de uso en móvil.
- `tabPosition="right"`: mismo coste funcional, pero deja la lista lejos del borde del diálogo y del
  gesto natural de lectura (izquierda → derecha).

### D2 — Sin ancho fijo para la lista lateral

**Decisión**: no se pasa `tabBarStyle`/`style` con `width` ni `minWidth` a ningún `Tabs`.

**Por qué**: rc-tabs dimensiona la lista al contenido en vertical. Fijar un ancho obliga a
sobrescribir estilos internos de antd (`.ant-tabs-nav`) y se rompe con el zoom del navegador o con
etiquetas nuevas más largas. El ancho útil del panel queda en ~800 px con el `Modal` a 1000 px, que
es más de lo que necesita ningún formulario actual.

### D3 — El ancho del `Modal` no cambia

**Decisión**: se mantienen los 1000 px.

**Por qué**: la orientación vertical libera ancho horizontal al quitar la fila superior, pero los
paneles con contenido ancho (tabla de skills, sub-pestañas de integraciones con URL base) agradecen
el ancho. El requisito que fijaba 960 px se retira porque su razón era el desbordamiento horizontal;
la cifra se conserva como decisión, ahora documentada en el requisito añadido.

### D4 — Verificación por `aria-orientation`, no por CSS

**Decisión**: los tests asertan `aria-orientation="vertical"` en el `tablist` relevante.

**Por qué**: es el atributo que rc-tabs pone en función de `tabPosition`, de modo que el test falla
si alguien quita el `tabPosition` (RED real, no cosmético) y no depende de nombres de clase
generados. Se localiza el `tablist` subiendo desde una pestaña conocida
(`tab.closest('[role="tablist"]')`), igual que ya hace el test de Integraciones.

### D5 — Retirar el requisito de desbordamiento en vez de reescribirlo

**Decisión**: `SettingsDialog SHALL fit all its top-level tabs without overflow` se retira; su
contenido útil (ancho de 1000 px y las diez pestañas) pasa al requisito añadido.

**Por qué**: reescribir un requisito cuyo nombre afirma lo contrario de lo que ahora se exige
(«quepan en una fila») deja el spec mintiendo en el índice. El texto además estaba obsoleto: citaba
«API Keys» y «Herramientas», pestañas que ya no existen.

## Risks / Trade-offs

- **Alto en pantallas bajas**: diez pestañas en vertical ocupan más alto que en horizontal y, en un
  portátil de 768 px, la lista puede necesitar scroll interno → antd desplaza el propio `tablist`
  y el `Modal` está acotado al viewport; se verifica en la tarea 4.2 con ventana estrecha. Si
  molesta, la mitigación es agrupar pestañas, no volver a horizontal.
- **Pérdida de ancho en el panel** (~200 px) → ningún formulario actual necesita más; se comprueba
  en 4.2 con «Skills» y «Generación», que son los paneles más densos.
- **Tests que asumían horizontal** → se localizan con `rg "aria-orientation|horizontal"` en
  `frontend/src/test/` antes del RED; hoy hay una sola aserción afectada (línea ~2722).
- **`forceRender` intacto**: los `Tabs` verticales siguen montando todos sus paneles donde ya lo
  hacían; no se relaja ni se endurece el montaje, así que los indicadores de Integraciones y los
  quince campos de Generación siguen funcionando igual.

## Migration Plan

Cambio solo de frontend, sin migración de datos ni de ajustes. Se despliega con el próximo release
de la imagen (`vX.Y.Z`) y el stack recreado desde Dockge; rollback = revertir el commit. No hay
estado persistido que deshacer.
