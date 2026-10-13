/**
 * Intervalo en el formato crudo de SQLite: `"YYYY-MM-DD HH:MM:SS"`, con
 * separador de **espacio**, sin `T`, sin `Z` y sin zona horaria. Admite
 * además fracciones de segundo (`".SSS"`), que `datetime('now')` no emite
 * pero que un `strftime` sí podría.
 */
const SQLITE_TIMESTAMP_RE = /^(\d{4}-\d{2}-\d{2}) (\d{2}:\d{2}:\d{2}(?:\.\d+)?)$/;

/**
 * Convierte un timestamp en un instante, normalizando el formato crudo de
 * SQLite a UTC.
 *
 * El backend envía `datetime('now')` tal cual sale de SQLite —`"YYYY-MM-DD
 * HH:MM:SS"`, sin `T`, sin `Z` y sin zona—, que NO es ISO-8601. `new Date()`
 * con esa cadena es ambiguo y peligroso:
 * - WebKit/Safari la rechaza → `Invalid Date` → la UI cae a `—`,
 *   indistinguible de «no hay fila».
 * - V8/Chrome la acepta pero como hora **local**, cuando el valor almacenado
 *   es **UTC** → el instante mostrado queda desfasado (2 h en Madrid en
 *   verano) y parece correcto.
 *
 * Aquí se reescribe el separador a `T` y se añade `Z` para fijar UTC, de modo
 * que el instante sea correcto y en pantalla se muestre en la hora local del
 * usuario. Si el valor ya viene en ISO-8601 (con `T` y zona) se respeta sin
 * tocarlo. Devuelve `null` si no es una fecha parseable.
 */
export const parseSqliteTimestamp = (value: string | null): Date | null => {
  if (!value) return null;
  const match = SQLITE_TIMESTAMP_RE.exec(value);
  const normalized = match ? `${match[1]}T${match[2]}Z` : value;
  const dt = new Date(normalized);
  return Number.isNaN(dt.getTime()) ? null : dt;
};
