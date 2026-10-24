# frontend/journal Specification

## Purpose
La sección de **Bitácora** de la interfaz de Valet: el tipo `JournalEntry`, los métodos de cliente de
`/api/journal`, una vista que lista las entradas de más reciente a más antigua con su ánimo, relato,
etiquetas, logros y dificultades —editable y borrable con confirmación— y su entrada en la navegación
de la aplicación.

## Requirements

### Requirement: El frontend SHALL definir el tipo `JournalEntry` y los métodos de la API de la bitácora

El frontend SHALL declarar en `frontend/src/types/` el tipo `JournalEntry` y el tipo `Mood`, y SHALL
exponer en `api/client.ts` los métodos `listJournalEntries`, `getJournalEntry`, `upsertJournalEntry` y
`deleteJournalEntry` para consumir `/api/journal`.

**Contracts:**

```typescript
// types/index.ts
export type Mood = "great" | "good" | "neutral" | "bad" | "terrible";

export interface JournalEntry {
  id: string;
  profile_id: string;
  date: string;              // YYYY-MM-DD
  mood: Mood | null;
  content: string;
  achievements: string[];
  struggles: string[];
  tags: string[];
  created_at: string;
  updated_at: string;
}

// api/client.ts
listJournalEntries: (params?: { from?: string; to?: string; limit?: number }) =>
  request<JournalEntry[]>("/journal?..."),
getJournalEntry: (date: string) => request<JournalEntry>(`/journal/${date}`),
upsertJournalEntry: (date: string, body: { content: string; mood?: Mood | null; achievements?: string[]; struggles?: string[]; tags?: string[] }) =>
  request<JournalEntry>(`/journal/${date}`, { method: "PUT", body: JSON.stringify(body) }),
deleteJournalEntry: (date: string) =>
  request<void>(`/journal/${date}`, { method: "DELETE" }),
```

**Scenarios:**

#### Scenario: Listar por rango

- **Given** el cliente de API del frontend
- **When** se invoca `listJournalEntries({ from, to, limit })`
- **Then** se solicita `GET /journal` con esos parámetros
- **And** la respuesta se tipa como `JournalEntry[]`

#### Scenario: Leer un día

- **Given** el cliente de API del frontend
- **When** se invoca `getJournalEntry("2026-10-18")`
- **Then** se solicita `GET /journal/2026-10-18`
- **And** la respuesta se tipa como `JournalEntry`

#### Scenario: Crear o reemplazar el día

- **Given** el cliente de API del frontend
- **When** se invoca `upsertJournalEntry("2026-10-18", { content, mood })`
- **Then** se envía un `PUT /journal/2026-10-18` con ese cuerpo
- **And** la respuesta se tipa como `JournalEntry`

#### Scenario: Borrar el día

- **Given** el cliente de API del frontend
- **When** se invoca `deleteJournalEntry("2026-10-18")`
- **Then** se envía un `DELETE /journal/2026-10-18`

### Requirement: La vista de Bitácora SHALL listar, editar y borrar las entradas

La interfaz SHALL exponer una vista de Bitácora que liste las entradas **de más reciente a más
antigua**, mostrando por cada una su fecha, su ánimo, su relato, sus etiquetas, sus logros y sus
dificultades. Cada entrada SHALL poder **editarse** —ánimo, relato y etiquetas— y **borrarse** con
una **confirmación** previa. La vista SHALL mostrar un estado de **carga** mientras carga, un estado
**vacío** cuando no haya entradas y un estado de **error** si la petición falla, como las demás
vistas del repositorio. Los textos SHALL estar en español.

**Scenarios:**

#### Scenario: La lista muestra las entradas de más reciente a más antigua

- **Given** entradas en varias fechas
- **When** se abre la vista de Bitácora
- **Then** muestra la entrada más reciente primero
- **And** cada entrada muestra su fecha, su ánimo, su relato, sus etiquetas, sus logros y sus dificultades

#### Scenario: Editar una entrada

- **Given** una entrada visible en la lista
- **When** el usuario cambia su ánimo, su relato y sus etiquetas y guarda
- **Then** se envía la actualización a la API y la lista refleja los cambios

#### Scenario: Borrar una entrada pide confirmación

- **Given** una entrada visible en la lista
- **When** el usuario pulsa borrar
- **Then** se le pide confirmación antes de borrar
- **And** solo al confirmar se llama a la API y la entrada desaparece de la lista

#### Scenario: Sin entradas se muestra el estado vacío

- **Given** que no hay entradas en la bitácora
- **When** se abre la vista de Bitácora
- **Then** se muestra un estado vacío

#### Scenario: Estado de carga

- **Given** la vista de Bitácora abierta mientras la petición está en curso
- **When** se renderiza
- **Then** se muestra un estado de carga

#### Scenario: Estado de error

- **Given** que la petición de la bitácora falla
- **When** se renderiza la vista
- **Then** se muestra un estado de error

### Requirement: `AppLayout` SHALL integrar la Bitácora en la navegación

`AppLayout` SHALL añadir en el header, junto a los botones de Tasks, Agenda, Stats y Avisos, una
entrada de navegación de **Bitácora** que abre una vista propia bajo `React.lazy` + `Suspense`,
siguiendo el molde de las demás secciones.

**Contracts:**

```typescript
// AppLayout.tsx
const JournalView = React.lazy(() => import("./JournalView").then((m) => ({ default: m.JournalView })));

const [journalVisible, setJournalVisible] = useState(false);

<Button type="text" icon={<BookOutlined />} aria-label="Bitácora" title="Bitácora"
        onClick={() => setJournalVisible(true)} />


<Modal title="📖 Bitácora" open={journalVisible} onCancel={() => setJournalVisible(false)} footer={null} width={1000}>
  <Suspense fallback={lazyViewFallback}>
    <JournalView />
  </Suspense>
</Modal>
```

**Scenarios:**

#### Scenario: La entrada está en el header y abre la Bitácora

- **Given** el `AppLayout` renderizado
- **When** el usuario pulsa la entrada de Bitácora del header
- **Then** se abre la vista de Bitácora

#### Scenario: La vista se carga de forma diferida

- **Given** el `AppLayout`
- **When** se inspecciona cómo se carga la vista de Bitácora
- **Then** se carga bajo `React.lazy` + `Suspense`, no en el bundle inicial
