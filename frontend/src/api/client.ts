import type {
  CalendarEvent,
  ChatInitResponse,
  CreateMessage,
  LastApiCall,
  MemoryStats,
  Message,
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
} from "../types";
import type { AuthUser } from "../contexts/AuthContext";

export const BASE_URL = "/api";

/** Evento global que emite el cliente al recibir un 401 en una ruta no-auth. */
export const UNAUTHORIZED_EVENT = "valet:unauthorized";

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

async function request<T>(path: string, options?: RequestInit): Promise<T> {
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

  listMessages: (limit = 50, cursor?: string) =>
    request<PaginatedResponse<Message>>(
      `/messages?limit=${limit}${cursor ? `&cursor=${cursor}` : ""}`,
    ),

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
  getStatsByModel: () => request<ModelStats[]>("/stats/llm/by-model"),
  getStatsBackground: () =>
    request<BackgroundStats[]>("/stats/llm/background"),
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
};
