/**
 * Construye el contenido de un turno de usuario que representa una acción de
 * widget. Formato estable de una sola línea:
 * `[widget:<name>#<id>] <action> <json-payload>`.
 */
export function formatWidgetAction(
  name: string,
  id: string,
  action: string,
  payload?: unknown,
): string {
  return `[widget:${name}#${id}] ${action} ${JSON.stringify(payload)}`;
}