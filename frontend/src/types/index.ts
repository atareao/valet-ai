import type { WidgetInstance } from "../components/widgets/types";

export interface Message {
  id: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  tool_calls?: unknown;
  tool_results?: unknown;
  tokens_count?: number;
  collapsed_content?: string | null;
  collapsed_tokens_count?: number;
  is_indexed?: boolean;
  summary_ref?: string | null;
  location?: string | null;
  tools_used?: string;
  created_at: string;
  widgets?: WidgetInstance[] | null;
}

export interface CreateMessage {
  role: string;
  content: string;
  tool_calls?: unknown;
  tool_results?: unknown;
  tools_used?: string;
}

export interface Settings {
  max_window_tokens: string;
  system_prompt: string;
  archivist_prompt: string;
  collapse_prompt: string;
  consolidator_prompt: string;
  [key: string]: string;
}

export interface Profile {
  id: string;
  name: string;
  avatar_url?: string;
  preferences: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface UpdateProfile {
  name?: string;
  avatar_url?: string;
  preferences?: Record<string, unknown>;
}

export interface PaginatedResponse<T> {
  data: T[];
  /**
   * Cursor de la página siguiente. El backend lo serializa como `null` cuando
   * no hay más resultados (`Option<String>`), no como campo ausente.
   */
  next_cursor?: string | null;
  total?: number;
}

/**
 * Aviso de la bandeja de notificaciones (`GET /api/notifications/unread`).
 * Espejo del contrato del backend: `metadata` puede ser `null` y `created_at`
 * llega en el formato crudo de SQLite (`"YYYY-MM-DD HH:MM:SS"`, UTC).
 */
export interface Notification {
  id: string;
  origin: string;
  author: string;
  message: string;
  level: "info" | "warning" | "critical";
  metadata: Record<string, unknown> | null;
  created_at: string;
}

export interface BrowserContext {
  timestamp: string;
  timezone: string;
  latitude: number | null;
  longitude: number | null;
  location_name: string | null;
}

export interface MessageQuery {
  content: string;
  browser_context?: BrowserContext;
  override?: string;
}

export type SSEEventType =
  | "chunk"
  | "tool_call"
  | "tool_result"
  | "widget"
  | "done"
  | "error"
  | "approval_required"
  | "approval_result";

export interface SSEStreamEvent {
  type: SSEEventType;
  content?: string;
  id?: string;
  name?: string;
  args?: unknown;
  data?: unknown;
  success?: boolean;
  message_id?: string;
  user_message_id?: string;
  location?: string | null;
  tools_used?: string;
  user_location?: string;
  user_created_at?: string;
  message?: string;
  request_id?: string;
  tool_name?: string;
  reason?: string;
  approved?: boolean;
}

export interface ChatInitResponse {
  messages: Message[];
  settings: Record<string, string>;
}

export interface Task {
  id: string;
  profile_id: string;
  content: string;
  status: "inbox" | "todo" | "doing" | "waiting" | "someday" | "done";
  priority: "low" | "medium" | "high";
  project?: string;
  due_date?: string;
  scope: "shared" | "personal";
  created_at: string;
  updated_at: string;
}

export interface StatsSummary {
  total_calls: number;
  total_prompt_tokens: number;
  total_completion_tokens: number;
  total_tokens: number;
  total_cached_tokens: number;
  total_reasoning_tokens: number;
  total_cost: number;
  total_errors: number;
  avg_duration_ms: number | null;
}

export interface ModelStats {
  model: string;
  calls: number;
  total_tokens: number;
  total_cost: number;
  avg_duration_ms: number | null;
  total_cached_tokens: number;
  total_reasoning_tokens: number;
}

/**
 * Uso de LLM de un origen no-chat (router, archivist, consolidator, collapse,
 * timeline). Espejo de `BackgroundStats` en `src/models/stats.rs`; el backend
 * devuelve **una entrada por pareja (origen, modelo)** desde
 * `GET /api/stats/llm/background`. `model` es `null` para un origen canónico
 * sin ninguna fila (contadores a cero): la visibilidad de que un rol nunca
 * corre es información, no ruido.
 */
export interface BackgroundStats {
  kind: string;
  model: string | null;
  calls: number;
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  total_cost: number;
  total_errors: number;
  /**
   * Llamadas con `status='success'` y `completion_tokens <= 1`: el «éxito con
   * forma de avería» que ni el código ni `total_errors` ven.
   */
  suspicious_calls: number;
  /** `MAX(created_at)` de la pareja; `null` si el origen no tiene filas. */
  last_call_at: string | null;
  avg_duration_ms: number | null;
}

export interface DayStats {
  date: string;
  calls: number;
  total_tokens: number;
  total_cost: number;
  total_cached_tokens: number;
  total_reasoning_tokens: number;
}

export interface ToolStats {
  tool: string;
  count: number;
}

export interface TableSize {
  table: string;
  rows: number;
}

export interface RetentionConfig {
  days: number;
}

export interface MemoryStats {
  total_memories: number;
  total_tokens: number;
  messages_indexed: number;
  messages_total: number;
}

export interface PersistentMemoryState {
  payload: Record<string, unknown> | null;
  updated_at: string | null;
  token_count: number;
  budget_tokens: number;
  ceiling_tokens: number;
  is_empty: boolean;
  warning?: string | null;
}

export interface LastApiCall {
  model: string;
  request_body: string | null;
  response_body: string | null;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  cached_tokens: number;
  reasoning_tokens: number;
  cost: number;
  duration_ms: number | null;
  status: string;
  error_message: string | null;
  tool_calls: string | null;
  created_at: string;
}

export interface CalendarEvent {
  id: string;
  profile_id: string;
  title: string;
  description?: string;
  start_time: string; // ISO 8601
  end_time: string;
  location?: string;
  scope: "shared" | "personal";
  category: "default" | "work" | "personal" | "health" | "birthday" | "holiday";
  all_day: boolean;
  rrule?: string;
  reminder_minutes_before?: number;
  created_at: string;
  updated_at: string;
}

/**
 * Campo de una skill que puede quedar sobrescrito por `settings`: la pregunta,
 * los dos criterios o el umbral. El fragmento de prompt no se marca (su
 * restauración equivale a vaciar su clave, no a un valor del catálogo).
 */
export type SkillOverrideField =
  | "question"
  | "criteria_true"
  | "criteria_false"
  | "threshold";

/** Skill del catálogo cerrado del enrutador (`GET /api/skills`). */
export interface SkillInfo {
  id: string;
  prompt_key: string;
  prompt_heading: string;
  tools: string[];
  /**
   * ¿Está habilitada? (`ROUTER_SKILL_<ID>_ENABLED`; por defecto sí). Opcional
   * para no romper fixtures antiguas del cliente; la UI lo trata como `true`
   * cuando falta.
   */
  enabled?: boolean;
  /** Pregunta efectiva (el valor de `settings` si lo tiene, si no el del catálogo). */
  question: string;
  /** Criterio del «sí» efectivo. */
  criteria_true: string;
  /** Criterio del «no» efectivo. */
  criteria_false: string;
  /** Umbral efectivo (override por skill si existe, si no el compilado). */
  threshold: number;
  /**
   * Campos cuyo valor efectivo difiere del catálogo: `question`,
   * `criteria_true`, `criteria_false` o `threshold`. Vacío si ninguno.
   */
  overridden: SkillOverrideField[];
}

/** Respuesta de `GET /api/skills`: skills enrutables + herramientas núcleo. */
export interface SkillsResponse {
  skills: SkillInfo[];
  core_tools: string[];
}

/**
 * Estado de la conexión con Strava (`GET /api/strava/status`). Espejo del
 * contrato del backend: **nunca** incluye tokens, solo si hay conexión y los
 * datos públicos del atleta autorizado.
 */
export interface StravaStatus {
  connected: boolean;
  athlete_id: string | null;
  athlete_name: string | null;
  scope: string | null;
}

/**
 * Resultado del sondeo activo de la conexión (`GET /api/strava/check`). La
 * ruta responde **siempre** `200`: `ok` indica si Strava atendió la petición y
 * `error` lleva un motivo accionable cuando no. Nunca incluye tokens.
 */
export interface StravaCheckResult {
  ok: boolean;
  athlete_id: string | null;
  athlete_name: string | null;
  error: string | null;
}

/** Respuesta de `GET /api/push/vapid-key`: la clave pública VAPID en base64url. */
export interface VapidKeyResponse {
  public_key: string;
}

/** Suscripción WebPush del navegador tal como la espera el backend. */
export interface PushSubscriptionPayload {
  endpoint: string;
  keys: {
    p256dh: string;
    auth: string;
  };
}

/** Respuesta de `POST /api/push/subscriptions`. */
export interface PushSubscriptionResponse {
  id: string;
}

/**
 * Clave de ingesta tal como la devuelve `GET /api/ingest-keys`. Nunca incluye el
 * token: solo su `prefix`.
 */
export interface IngestKey {
  id: string;
  name: string;
  prefix: string;
  created_at: string;
  last_used_at: string | null;
  revoked_at: string | null;
}

/**
 * Respuesta de `POST /api/ingest-keys`: el token en claro se entrega **una sola
 * vez**, en el alta. No vuelve a estar disponible en ningún listado.
 */
export interface CreatedIngestKey extends IngestKey {
  token: string;
}
