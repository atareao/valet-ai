-- Registro de idempotencia de las respuestas a comentarios del blog: una fila
-- por comentario respondido (o con una respuesta en curso).
--
-- Aditiva e idempotente: `CREATE TABLE IF NOT EXISTS` más un índice con
-- `IF NOT EXISTS`, así que reejecutar el fichero tal cual es un no-op y ninguna
-- tabla existente se reconstruye ni se ensancha. La tabla nace vacía; no hay
-- datos que migrar.
--
-- `comment_id` (el id del comentario al que se responde, en su forma decimal) es
-- la clave primaria: es lo que hace de la respuesta una operación «una sola vez».
-- La reserva se toma con un único `INSERT ... ON CONFLICT(comment_id) DO NOTHING`,
-- de modo que una sola llamada concurrente gana y las demás ven cero filas
-- afectadas y no salen a la red.
--
-- `state` es el estado de la publicación, uno de `'reserved'` (reserva tomada y
-- aún sin desenlace), `'published'` (WordPress creó la respuesta) o `'uncertain'`
-- (desenlace ambiguo: timeout, fallo de red o 5xx; la reserva se conserva para no
-- publicar dos veces). El `CHECK` es la puerta del conjunto cerrado.
--
-- `wp_comment_id` es el id que devuelve WordPress al crear la respuesta y
-- `last_status` el código HTTP del último intento; ambos son informativos.
--
-- ── Rescate manual de una reserva bloqueada ─────────────────────────────────
--
-- Una reserva `reserved` o `uncertain` bloquea una segunda publicación a
-- propósito (es preferible no responder a responder dos veces). Si el operador
-- comprueba que el comentario NO se creó, la desbloquea con esta orden exacta,
-- sustituyendo `<id>` por el `comment_id` en su forma decimal:
--
--     DELETE FROM comment_replies WHERE comment_id = '<id>';
--
-- Con la fila borrada, la siguiente invocación para ese comentario vuelve a
-- reservar y a publicar. La herramienta no reintenta sola en ningún caso.
CREATE TABLE IF NOT EXISTS comment_replies (
    comment_id TEXT PRIMARY KEY NOT NULL,
    post_id TEXT NOT NULL,
    notification_id TEXT,
    state TEXT NOT NULL CHECK (state IN ('reserved', 'published', 'uncertain')),
    wp_comment_id TEXT,
    last_status INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- El listado «qué comentarios de la entrada X ya respondí» es lo que sirve este
-- índice sin ordenar la tabla entera.
CREATE INDEX IF NOT EXISTS idx_comment_replies_post
    ON comment_replies(post_id, created_at);
