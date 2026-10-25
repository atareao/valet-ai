-- El chat no razona por defecto: corrige a `off` el vacío heredado (default
-- anterior). Respeta cualquier valor no vacío elegido por el usuario.
UPDATE settings
SET value = 'off', updated_at = datetime('now')
WHERE key = 'GENERATION_CHAT_REASONING' AND value = '';
