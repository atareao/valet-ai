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
  DayStats,
  ToolStats,
  TableSize,
  RetentionConfig,
  Task,
  UpdateProfile,
  PersistentMemoryState,
  Tool,
} from "../types";

export const BASE_URL = "/api";

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
    headers: { "Content-Type": "application/json", ...options?.headers },
    ...options,
  });
  if (!resp.ok) {
    const error = await resp.json().catch(() => ({ error: resp.statusText }));
    throw new ApiError(error.error || `HTTP ${resp.status}`, resp.status);
  }
  if (resp.status === 204) return undefined as T;
  return resp.json();
}

export const api = {
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

  getTools: () => request<Tool[]>("/tools"),

  toggleTool: (id: string) =>
    request<Tool>(`/tools/${id}/toggle`, { method: "PUT" }),
};
