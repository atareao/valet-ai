# frontend-lint Specification

## Purpose
Salud del código del frontend: efectos de carga sin `setState` síncrono, dependencias de hooks completas y estables, y el presupuesto de ESLint en cero.

## Requirements

### Requirement: Efectos de carga sin `setState` síncrono

`useEvents`, `useTasks`, `useSettings`, `useMainChat` y `StatsDashboard` SHALL NOT ejecutar `setState`
de forma síncrona desde el cuerpo de un `useEffect`. Las marcas de "petición en curso" SHALL fijarse
fuera del camino síncrono del efecto y las actualizaciones de estado SHALL producirse al resolverse
la petición. NO SHALL añadirse directivas `eslint-disable` para silenciar
`react-hooks/set-state-in-effect`.

**Given** cualquiera de los cinco puntos de carga
**When** se ejecuta `npx eslint .`
**Then** NO SHALL emitirse ningún `react-hooks/set-state-in-effect` en ellos
**And** NO SHALL existir ninguna directiva `eslint-disable` para esa regla

#### Scenario: El montaje no provoca render en cascada

**Given** el hook `useEvents` con la API mockeada
**When** el hook se monta por primera vez
**Then** SHALL dispararse exactamente una petición
**And** NO SHALL invocarse `setState` de forma síncrona durante el cuerpo del efecto
**And** `loading` SHALL valer `true` hasta que la petición se resuelva

#### Scenario: La carga inicial sigue ocurriendo

**Given** el hook `useSettings` con la API mockeada
**When** el componente se monta
**Then** `api.getSettings` SHALL ser invocado
**And** `settings` SHALL contener los datos cuando la promesa se resuelve
**And** `loading` SHALL pasar a `false`

#### Scenario: `refetch` manual sigue funcionando

**Given** el hook `useEvents` ya montado
**When** se invoca `refetch()` desde un manejador de evento
**Then** SHALL lanzarse una nueva petición
**And** `events` SHALL actualizarse con la respuesta

#### Scenario: La recarga por evento sigue funcionando

**Given** el hook `useTasks` montado
**When** se emite `window.dispatchEvent(new Event("tasks-changed"))`
**Then** SHALL lanzarse una nueva petición a `api.listTasks`

#### Scenario: Al cambiar las dependencias se relanza la carga

**Given** el hook `useEvents` montado con `start` y `end`
**When** cambia `start` o `end`
**Then** SHALL lanzarse una nueva petición con los valores nuevos

### Requirement: Dependencias de efectos completas y estables

`useMediaQuery` y `ChatView` SHALL declarar dependencias de efecto completas y estables: los valores
derivados usados dentro de un `useEffect` SHALL envolverse en `useCallback` o `useMemo` para que su
identidad no cambie en cada render. NO SHALL usarse `eslint-disable` para silenciar
`react-hooks/exhaustive-deps`.

#### Scenario: `useMediaQuery` sin dependencias ausentes

**Given** `useMediaQuery`
**When** se ejecuta `npx eslint .`
**Then** NO SHALL emitirse `react-hooks/exhaustive-deps` para `useMediaQuery.ts`
**And** el hook SHALL seguir reaccionando a cambios de `matchMedia`

#### Scenario: `ChatView` con valor derivado estable

**Given** `ChatView` con su lista condicional de mensajes
**When** se ejecuta `npx eslint .`
**Then** NO SHALL emitirse `react-hooks/exhaustive-deps` para `ChatView.tsx`
**And** el valor derivado SHALL ser estable entre renders con las mismas entradas

### Requirement: Los módulos de componente exportan solo componentes

`CalendarView.tsx` SHALL exportar únicamente componentes, de modo que el fast refresh funcione. Las
constantes compartidas SHALL residir en módulos propios.

#### Scenario: Fast refresh en `CalendarView`

**Given** el módulo `CalendarView.tsx`
**When** se ejecuta `npx eslint .`
**Then** NO SHALL emitirse `react-refresh/only-export-components`
**And** las constantes SHALL importarse desde un módulo distinto
**And** la UI SHALL comportarse igual

### Requirement: Sin directivas `eslint-disable` inertes

`useSSE.ts` SHALL NOT conservar la directiva `eslint-disable-next-line no-constant-condition` si ya
no suprime ningún diagnóstico.

#### Scenario: Directiva muerta eliminada

**Given** `useSSE.ts`
**When** se ejecuta `npx eslint .`
**Then** NO SHALL emitirse `Unused eslint-disable directive`
**And** la directiva SHALL haber sido eliminada

### Requirement: El presupuesto de lint es cero

El job `Frontend (Node)` del CI SHALL ejecutar ESLint con `--max-warnings 0`.

#### Scenario: El árbol limpio pasa

**Given** el árbol tras este change
**When** se ejecuta `npx eslint . --max-warnings 0`
**Then** SHALL salir con código 0 y cero problemas

#### Scenario: Un warning nuevo rompe el CI

**Given** el workflow `ci.yml` con `--max-warnings 0`
**When** se introduce un warning nuevo
**Then** el job `Frontend (Node)` SHALL fallar

### Requirement: El refactor no cambia el comportamiento observable

Las 116 pruebas existentes SHALL pasar sin modificar ninguna de sus aserciones. Los módulos sin
cobertura directa que se refactorizan (`useTasks`, `CalendarView`, `TaskView`) SHALL quedar fijados
por pruebas de caracterización añadidas **antes** de tocarlos.

#### Scenario: Ninguna aserción existente cambia

**Given** el diff del change
**When** se inspeccionan los ficheros `*.test.ts(x)` preexistentes
**Then** NO SHALL aparecer ninguna modificación en ellos
**And** SHALL haberse añadido pruebas de caracterización nuevas

#### Scenario: Suite completa en verde

**Given** el árbol tras el change
**When** se ejecuta `npx vitest run`
**Then** SHALL pasar el 100% de las pruebas
**And** `npx tsc --noEmit` SHALL salir con código 0
