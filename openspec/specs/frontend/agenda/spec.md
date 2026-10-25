# frontend/agenda Specification

## Purpose
Interfaz de agenda de Valet: componente de calendario con vistas de día, semana y mes, creación, edición y eliminación de eventos, categorías con colores y filtrado por categoría.

## Requirements

### Requirement: Calendar view component

El componente SHALL mostrar un calendario de Ant Design con vistas mensual, semanal y diaria,
permitiendo crear, editar y eliminar eventos y filtrarlos por categoría. El **borrado** SHALL
ejecutarse de verdad: al confirmar en el detalle del evento, la vista SHALL llamar a
`api.deleteEvent(id)` (a través de `useDeleteEvent`), SHALL cerrar el detalle y SHALL recargar el
listado (`refetch`) cuando el borrado tenga éxito. Si el borrado **falla**, la vista SHALL mostrar
un aviso de error y el modal de detalle SHALL permanecer abierto (no SHALL cerrarse ni recargarse
fingiendo el borrado).

New React component for agenda visualization using Ant Design Calendar with day/week/month views.

**Contracts:**

```typescript
// Types
interface CalendarEvent {
  id: string;
  title: string;
  description?: string;
  start: string;          // ISO 8601
  end: string;
  location?: string;
  scope: 'shared' | 'personal';
  category: 'default' | 'work' | 'personal' | 'health' | 'birthday' | 'holiday';
  allDay: boolean;
  rrule?: string;
}

// Category color mapping
const CATEGORY_COLORS: Record<string, string> = {
  default: '#1677ff',     // blue
  work: '#52c41a',        // green
  personal: '#fa8c16',    // orange
  health: '#f5222d',      // red
  birthday: '#eb2f96',    // pink
  holiday: '#722ed1',     // purple
};

// Hooks
function useEvents(dateRange: [string, string]): {
  events: CalendarEvent[];
  loading: boolean;
  error: Error | null;
  refetch: () => void;
}

function useCreateEvent(): {
  create: (event: Partial<CalendarEvent>) => Promise<CalendarEvent>;
  loading: boolean;
}

function useUpdateEvent(): {
  update: (id: string, changes: Partial<CalendarEvent>) => Promise<void>;
  loading: boolean;
}

function useDeleteEvent(): {
  delete: (id: string) => Promise<void>;
  loading: boolean;
}
```

**Scenarios:**

#### Scenario: Calendar view shows monthly grid
Given the user navigates to the agenda section
When the CalendarView component renders
Then it shows a monthly calendar grid with Ant Design Calendar
And events are displayed as colored badges on their dates

#### Scenario: Click on date shows daily events
Given the CalendarView is in monthly mode
When the user clicks on a specific day
Then the view switches to daily detail showing that day's events with times

#### Scenario: Create event from calendar
Given the CalendarView is open
When the user clicks "New Event" button
Then a modal/form opens with fields: title, start, end, allDay, category, scope, rrule, reminder
And on submit the event is created and appears in the calendar

#### Scenario: Edit event from calendar
Given an existing event displayed in the calendar
When the user clicks on the event
Then a modal opens with pre-filled fields
And on save the event is updated

#### Scenario: Delete event from calendar
**Given** un evento existente mostrado en el detalle del calendario
**When** el usuario pulsa «Delete» y confirma
**Then** SHALL llamarse a `deleteEvent` con el `id` del evento
**And** al terminar con éxito SHALL cerrarse el detalle y recargarse el listado
**And** el evento SHALL dejar de aparecer

#### Scenario: Un borrado fallido avisa y no finge el borrado
**Given** un evento existente mostrado en el detalle
**When** el usuario confirma el borrado y `deleteEvent` **rechaza**
**Then** SHALL mostrarse un aviso de error
**And** el modal de detalle SHALL permanecer abierto
**And** NO SHALL recargarse el listado como si se hubiera borrado

#### Scenario: Filter events by category
Given the calendar has events of multiple categories
When the user selects a filter for "work" category
Then only work events are displayed
