import type {
  CalendarEvent,
  ChatCursor,
  ChatInitResponse,
  CreateMessage,
  JournalEntry,
  JournalEntryInput,
  LastApiCall,
  MemoryStats,
  Message,
  Note,
  NoteCategory,
  NoteInput,
  Notification,
  PaginatedResponse,
  Profile,
  StatsSummary,
  ModelStats,
  BackgroundStats,
  DayStats,
  ToolStats,
  TableSize,
  RetentionConfig,
  Task,
  UpdateProfile,
  PersistentMemoryState,
  SkillsResponse,
  StravaCheckResult,
  StravaStatus,
} from "../types";
import type { AuthUser } from "../contexts/AuthContext";

export const BASE_URL = "/api";

/** Evento global que emite el cliente al recibir un 401 en una ruta no-auth. */
export const UNAUTHORIZED_EVENT = "valet:unauthorized";

/**
 * URL del bus de eventos del chat (`GET /api/chat/events`).
 *
 * `since` es el cursor **opaco** del último mensaje visto y va **codificado**:
 * su forma de cable contiene `|` (inválido en un request-target) y `+` (que el
 * servidor decodifica como espacio). Sin codificar, el stream responde `400`.
 */
export function chatEventsUrl(since?: ChatCursor | null): string {
  const qs = new URLSearchParams();
  if (since) qs.set("since", since);
  const suffix = qs.toString();
  return `${BASE_URL}/chat/events${suffix ? `?${suffix}` : ""}`;
}

/**
 * Las rutas de autenticación (`/auth/*`) quedan exentas de la detección global
 * de 401: un 401 en `me`/`login`/`logout` es parte del propio flujo y no debe
 * disparar una redirección sobre otra petición.
 */
function isAuthPath(path: string): boolean {
  return path.startsWith("/auth/");
}

/**
 * Error de la API que conserva el código HTTP. Los consumidores que solo
 * necesitan el mensaje siguen tratándolo como un `Error` normal.
 */
export class ApiError extends Error {
  readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

export async function request<T>(
  path: string,
  options?: RequestInit,
): Promise<T> {
  const resp = await fetch(`${BASE_URL}${path}`, {
    ...options,
    credentials: "include",
    headers: { "Content-Type": "application/json", ...options?.headers },
  });
  if (!resp.ok) {
    if (resp.status === 401 && !isAuthPath(path)) {
      window.dispatchEvent(new Event(UNAUTHORIZED_EVENT));
    }
    const error = await resp.json().catch(() => ({ error: resp.statusText }));
    throw new ApiError(error.error || `HTTP ${resp.status}`, resp.status);
  }
  if (resp.status === 204) return undefined as T;
  return resp.json();
}

export const api = {
  getMe: () => request<AuthUser>("/auth/me"),

  logout: () =>
    // Contrato backend (`src/routes/auth.rs` → `LogoutResponse`):
    // `end_session_url` es `null` cuando solo se cierra la sesión local.
    request<{ end_session_url: string | null }>("/auth/logout", {
      method: "POST",
    }),

  chatInit: () => request<ChatInitResponse>("/chat/init"),

  // El `cursor` es opaco (su forma de cable lleva `|` y `+`): va codificado o
  // la paginación se rompe en silencio o el backend responde `400`.
  listMessages: (limit = 50, cursor?: string) => {
    const qs = new URLSearchParams();
    qs.set("limit", String(limit));
    if (cursor) qs.set("cursor", cursor);
    return request<PaginatedResponse<Message>>(`/messages?${qs.toString()}`);
  },

  createMessage: (data: CreateMessage) =>
    request<Message>("/messages", {
      method: "POST",
      body: JSON.stringify(data),
    }),

  getProfile: () => request<Profile>("/profile"),
  updateProfile: (data: UpdateProfile) =>
    request<Profile>("/profile", { method: "PUT", body: JSON.stringify(data) }),

  getSettings: () => request<Record<string, string>>("/settings"),

  updateSettings: (data: Record<string, string>) =>
    request<Record<string, string>>("/settings", {
      method: "PUT",
      body: JSON.stringify(data),
    }),

  approveAction: (requestId: string, approved: boolean) =>
    request<void>(`/approval/${requestId}`, {
      method: "POST",
      body: JSON.stringify({ approved }),
    }),

  listEvents: (start: string, end: string) =>
    request<CalendarEvent[]>(
      `/events?start=${encodeURIComponent(start)}&end=${encodeURIComponent(end)}`,
    ),

  createEvent: (data: Partial<CalendarEvent>) =>
    request<CalendarEvent>("/events", {
      method: "POST",
      body: JSON.stringify(data),
    }),

  updateEvent: (id: string, data: Partial<CalendarEvent>) =>
    request<CalendarEvent>(`/events/${id}`, {
      method: "PUT",
      body: JSON.stringify(data),
    }),

  deleteEvent: (id: string) =>
    request<void>(`/events/${id}`, { method: "DELETE" }),

  listTasks: (params?: Record<string, string>) => {
    const qs = params ? "?" + new URLSearchParams(params).toString() : "";
    return request<Task[]>(`/tasks${qs}`);
  },

  createTask: (data: Partial<Task>) =>
    request<Task>("/tasks", { method: "POST", body: JSON.stringify(data) }),

  updateTask: (id: string, data: Partial<Task>) =>
    request<Task>(`/tasks/${id}`, {
      method: "PUT",
      body: JSON.stringify(data),
    }),

  deleteTask: (id: string) =>
    request<Task>(`/tasks/${id}`, { method: "DELETE" }),

  getStatsSummary: () => request<StatsSummary>("/stats/llm/summary"),
  getStatsByModel: (days = 30) =>
    request<ModelStats[]>(`/stats/llm/by-model?days=${days}`),
  getStatsBackground: (days = 30) =>
    request<BackgroundStats[]>(`/stats/llm/background?days=${days}`),
  getStatsByDay: (days = 30) => request<DayStats[]>(`/stats/llm/by-day?days=${days}`),
  getStatsTools: () => request<ToolStats[]>("/stats/llm/tools"),
  getDbSizes: () => request<TableSize[]>("/stats/db/sizes"),
  exportStatsCsv: () => {
    window.open(`${BASE_URL}/stats/llm/export`, "_blank");
  },
  getRetention: () => request<RetentionConfig>("/stats/retention"),
  setRetention: (days: number) =>
    request<RetentionConfig>("/stats/retention", {
      method: "PUT",
      body: JSON.stringify({ days }),
    }),
  getMemoryStats: () => request<MemoryStats>("/stats/memory"),
  getLastApiCall: () => request<LastApiCall | null>("/stats/llm/last-call"),

  getPersistentMemory: () =>
    request<PersistentMemoryState>("/persistent-memory"),

  updatePersistentMemory: (
    payload: Record<string, unknown>,
    expectedUpdatedAt: string | null,
  ) =>
    request<PersistentMemoryState>("/persistent-memory", {
      method: "PUT",
      body: JSON.stringify({
        payload,
        expected_updated_at: expectedUpdatedAt,
      }),
    }),

  clearPersistentMemory: () =>
    request<void>("/persistent-memory", { method: "DELETE" }),

  getSkills: () => request<SkillsResponse>("/skills"),

  // --- Avisos (bandeja de notificaciones) ---------------------------------
  // `GET /api/notifications/unread` solo lista los pendientes (no leídos), con
  // paginación por cursor. `cursor` ausente = primera página.
  listUnreadNotifications: (params?: { limit?: number; cursor?: string }) => {
    const qs = new URLSearchParams();
    if (params?.limit !== undefined) qs.set("limit", String(params.limit));
    if (params?.cursor) qs.set("cursor", params.cursor);
    const suffix = qs.toString();
    return request<PaginatedResponse<Notification>>(
      `/notifications/unread${suffix ? `?${suffix}` : ""}`,
    );
  },
  markNotificationsRead: (ids: string[]) =>
    request<{ marked: number }>("/notifications/mark-read", {
      method: "POST",
      body: JSON.stringify({ ids }),
    }),
  // `{}` (sin `ids`) es lo que hace que el backend marque **todas**.
  markAllNotificationsRead: () =>
    request<{ marked: number }>("/notifications/mark-read", {
      method: "POST",
      body: JSON.stringify({}),
    }),

  // --- Strava -------------------------------------------------------------
  // El flujo OAuth se inicia **navegando** a `/api/strava/authorize` (el
  // backend responde 302 hacia Strava), no con `fetch`. Los tokens jamás
  // cruzan el cliente: `status` solo devuelve el atleta y el scope.
  getStravaStatus: () => request<StravaStatus>("/strava/status"),

  // Sondeo activo del estado real de la conexión. **Siempre** `200`: el
  // diagnóstico viaja en `ok`/`error`, por eso no rechaza ante un fallo de
  // Strava (solo si la propia petición HTTP falla).
  checkStrava: () => request<StravaCheckResult>("/strava/check"),

  disconnectStrava: () =>
    // `warning` solo llega cuando la revocación remota no se pudo confirmar; se
    // omite del JSON cuando no hay aviso.
    request<{ connected: boolean; warning?: string | null }>(
      "/strava/disconnect",
      { method: "POST" },
    ),

  // --- Bitácora ------------------------------------------------------------
  // `GET /api/journal` lista por rango **inclusivo**, de la entrada más reciente
  // a la más antigua. Sin parámetros devuelve el diario entero: los extremos los
  // pone el backend.
  listJournalEntries: (params?: { from?: string; to?: string; limit?: number }) => {
    const qs = new URLSearchParams();
    if (params?.from) qs.set("from", params.from);
    if (params?.to) qs.set("to", params.to);
    if (params?.limit !== undefined) qs.set("limit", String(params.limit));
    const suffix = qs.toString();
    return request<JournalEntry[]>(`/journal${suffix ? `?${suffix}` : ""}`);
  },

  // Un día sin entrada responde `404` con `{"error": "no journal entry for …"}`.
  getJournalEntry: (date: string) => request<JournalEntry>(`/journal/${date}`),

  // `PUT` **reemplaza** la entrada del día: lo que no venga en el cuerpo se
  // guarda como lista vacía. Quien edite debe mandar también lo que no toca.
  upsertJournalEntry: (date: string, body: JournalEntryInput) =>
    request<JournalEntry>(`/journal/${date}`, {
      method: "PUT",
      body: JSON.stringify(body),
    }),

  deleteJournalEntry: (date: string) =>
    request<void>(`/journal/${date}`, { method: "DELETE" }),

  // --- Notas ---------------------------------------------------------------
  // `GET /api/notes` lista las notas del perfil, de la más reciente a la más
  // antigua. `category` filtra por una de las tres válidas; `limit` por defecto
  // 100 y máximo 500 (fuera de rango → `400`).
  listNotes: (params?: { category?: NoteCategory; limit?: number }) => {
    const qs = new URLSearchParams();
    if (params?.category) qs.set("category", params.category);
    if (params?.limit !== undefined) qs.set("limit", String(params.limit));
    const suffix = qs.toString();
    return request<Note[]>(`/notes${suffix ? `?${suffix}` : ""}`);
  },

  // `tags` viaja como cadena coma-separada (o ausente); `content` es obligatorio.
  createNote: (data: NoteInput) =>
    request<Note>("/notes", { method: "POST", body: JSON.stringify(data) }),

  // `PUT` actualiza de forma **parcial**: un campo ausente deja el valor
  // anterior y `tags: ""` vacía las etiquetas (la cadena vacía es un valor).
  updateNote: (id: string, data: NoteInput) =>
    request<Note>(`/notes/${id}`, {
      method: "PUT",
      body: JSON.stringify(data),
    }),

  deleteNote: (id: string) =>
    request<void>(`/notes/${id}`, { method: "DELETE" }),
};
