# frontend/notifications Specification

## Purpose
Bandeja de avisos del panel de Valet: el tipo `Notification` y los métodos de cliente para listar las no leídas (`listUnreadNotifications`) y marcarlas como leídas (`markNotificationsRead`, `markAllNotificationsRead`), el hook `useNotifications` con paginación por pila de cursores, la vista `NotificationsView` que lista los pendientes de 10 en 10 y permite marcarlos como leídos, y su integración en el header de `AppLayout` mediante un botón de avisos que abre un modal.

## Requirements

### Requirement: El frontend SHALL definir el tipo `Notification` y los métodos de la API de avisos

El frontend SHALL declarar el tipo `Notification` y SHALL exponer los métodos de cliente
`listUnreadNotifications`, `markNotificationsRead` y `markAllNotificationsRead` para consumir la
bandeja de avisos.

**Contracts:**

```typescript
// types/index.ts
export interface Notification {
  id: string;
  origin: string;
  author: string;
  message: string;
  level: "info" | "warning" | "critical";
  metadata: Record<string, unknown> | null;
  created_at: string;
}

// api/client.ts
listUnreadNotifications: (params?: { limit?: number; cursor?: string }) =>
  request<PaginatedResponse<Notification>>("/notifications/unread?..."),
markNotificationsRead: (ids: string[]) =>
  request<{ marked: number }>("/notifications/mark-read", { method: "POST", body: JSON.stringify({ ids }) }),
markAllNotificationsRead: () =>
  request<{ marked: number }>("/notifications/mark-read", { method: "POST", body: JSON.stringify({}) }),
```

**Scenarios:**

#### Scenario: Listar no leídas con `limit` y `cursor`

- **Given** el cliente de API del frontend
- **When** se invoca `listUnreadNotifications({ limit: 10, cursor })`
- **Then** se solicita `GET /notifications/unread` con ese `limit` y ese `cursor`
- **And** la respuesta se tipa como `PaginatedResponse<Notification>`

#### Scenario: Marcar avisos por ids

- **Given** una lista de ids de avisos
- **When** se invoca `markNotificationsRead(ids)`
- **Then** se envía un `POST /notifications/mark-read` con el cuerpo `{ ids }`
- **And** la respuesta se tipa como `{ marked: number }`

#### Scenario: Marcar todas con cuerpo vacío

- **Given** el cliente de API del frontend
- **When** se invoca `markAllNotificationsRead()`
- **Then** se envía un `POST /notifications/mark-read` con el cuerpo `{}`
- **And** la respuesta se tipa como `{ marked: number }`

### Requirement: El frontend SHALL exponer el hook `useNotifications`

El frontend SHALL exponer el hook `useNotifications(pageSize = 10)`, que gestiona la paginación de
los avisos manteniendo una **pila de cursores** para poder avanzar y retroceder entre páginas.

**Contracts:**

```typescript
// hooks/useNotifications.ts
function useNotifications(pageSize = 10): {
  notifications: Notification[];
  total: number;
  loading: boolean;
  error: string | null;
  page: number;
  hasPrev: boolean;
  hasNext: boolean;
  goNext: () => void;
  goPrev: () => void;
  refetch: () => void;
  markRead: (id: string) => Promise<void>;
  markAll: () => Promise<void>;
}
```

**Scenarios:**

#### Scenario: Carga la primera página

- **Given** avisos no leídos en el backend
- **When** se monta `useNotifications`
- **Then** carga la primera página y expone `notifications` y `total`

#### Scenario: `goNext` avanza y `goPrev` retrocede

- **Given** el hook en la primera página con `hasNext` verdadero
- **When** se llama a `goNext` y luego a `goPrev`
- **Then** `goNext` trae la página siguiente y `goPrev` vuelve a la anterior

#### Scenario: `markRead` marca y refresca

- **Given** un aviso visible en la lista
- **When** se invoca `markRead(id)`
- **Then** el aviso se marca como leído en el backend
- **And** la lista se refresca

#### Scenario: `markAll` marca todas y deja la lista vacía

- **Given** avisos no leídos en la lista
- **When** se invoca `markAll()`
- **Then** todas se marcan como leídas
- **And** la lista queda vacía

### Requirement: `NotificationsView` SHALL listar los avisos pendientes de 10 en 10 y permitir marcarlos como leídos

`NotificationsView` SHALL listar solo los avisos **no leídos**, de **10 en 10**, con controles
«Anterior» y «Siguiente» (este último deshabilitado cuando no haya `next_cursor`) y un recuento de
pendientes (`total`). Cada fila SHALL mostrar el origen, el autor, el nivel (con color según `info`,
`warning` o `critical`), el mensaje y `created_at`, y SHALL ofrecer la acción «Marcar como leído».
La vista SHALL ofrecer además un botón «Marcar todos como leídos» y SHALL mostrar un estado vacío
cuando no queden pendientes. Las fechas SHALL interpretarse como **UTC** cuando lleguen en el
formato crudo de SQLite (`"YYYY-MM-DD HH:MM:SS"`: separador de espacio, sin `T` y sin zona horaria),
SHALL mostrarse en la hora **local** del navegador dentro de un elemento `<time>` cuyo atributo
`dateTime` SHALL exponer el instante UTC, y SHALL mostrarse como `—` (sin `<time>`) si no son
parseables, sin mostrar nunca el valor crudo ni «Invalid Date» (WebKit rechaza el crudo; V8 lo lee
como hora local).

**Scenarios:**

#### Scenario: Página de 10 con Siguiente y Anterior

- **Given** más de 10 avisos no leídos
- **When** se abre `NotificationsView`
- **Then** muestra 10 avisos y el recuento de pendientes
- **And** «Siguiente» está habilitado y «Anterior» deshabilitado en la primera página

#### Scenario: Marcar uno lo quita de la lista

- **Given** un aviso visible en la lista
- **When** el usuario pulsa «Marcar como leído» en su fila
- **Then** el aviso se marca como leído y desaparece de la lista de pendientes

#### Scenario: Marcar todos vacía la lista y muestra el estado vacío

- **Given** avisos pendientes en la lista
- **When** el usuario pulsa «Marcar todos como leídos»
- **Then** la lista queda vacía
- **And** se muestra el estado vacío

#### Scenario: La fecha se interpreta como UTC y se muestra en la hora local

- **Given** un aviso con `created_at` en el formato crudo de SQLite `"2026-10-12 13:55:33"`
- **When** se renderiza la vista
- **Then** ese instante SHALL interpretarse como UTC (el instante correcto)
- **And** el texto SHALL mostrarse en la hora **local** del navegador
- **And** el elemento `<time>` SHALL exponer el instante UTC en su atributo `dateTime`

#### Scenario: Una fecha no parseable se muestra como ausencia

- **Given** un aviso con `created_at` no parseable
- **When** se renderiza la vista
- **Then** se muestra `—` (sin `<time>`)
- **And** nunca se muestra «Invalid Date» ni el valor crudo

#### Scenario: Sin pendientes se muestra el estado vacío

- **Given** que no hay avisos no leídos
- **When** se abre `NotificationsView`
- **Then** se muestra el estado vacío

### Requirement: `AppLayout` SHALL integrar el botón de avisos

`AppLayout` SHALL añadir en el header, junto a los botones de Tasks, Agenda y Stats, un botón de
avisos con el icono `BellOutlined` que abre un `Modal title="🔔 Avisos"` con `NotificationsView`
bajo `React.lazy` + `Suspense`, siguiendo el molde del modal de `TaskView`.

**Contracts:**

```typescript
// AppLayout.tsx
const NotificationsView = React.lazy(() => import('./NotificationsView'));

const [notificationsVisible, setNotificationsVisible] = useState(false);

// Nuevo botón en el header, junto a Tasks/Agenda/Stats
<Button type="text" icon={<BellOutlined />} onClick={() => setNotificationsVisible(true)} />

// Nuevo modal
<Modal title="🔔 Avisos" open={notificationsVisible} onCancel={() => setNotificationsVisible(false)} footer={null} width={1000}>
  <Suspense fallback={<Spin />}>
    <NotificationsView />
  </Suspense>
</Modal>
```

**Scenarios:**

#### Scenario: El botón está en el header y abre el diálogo

- **Given** el `AppLayout` renderizado
- **When** el usuario pulsa el botón de avisos del header
- **Then** se abre el modal «🔔 Avisos» con `NotificationsView`

#### Scenario: Al abrir se cargan los avisos

- **Given** avisos no leídos en el backend
- **When** se abre el modal de avisos
- **Then** `NotificationsView` carga y lista los avisos pendientes
