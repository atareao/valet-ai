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
  next_cursor?: string;
  total?: number;
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

export interface Tool {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
}
