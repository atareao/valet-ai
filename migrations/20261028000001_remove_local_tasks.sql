-- Retira el TODO local: las tareas viven en OpenTodo desde la fase 1 y las filas
-- existentes se migraron antes de este release.
-- `tasks_fts` es una tabla FTS5 de contenido externo sobre `tasks`, así que se
-- elimina primero.
DROP TABLE IF EXISTS tasks_fts;
DROP TABLE IF EXISTS tasks;
