# Spec Delta: db/repos

## MODIFIED Requirements

### Requirement: StatsRepo SHALL provide database table sizes

El repositorio SHALL exponer `StatsRepo::db_sizes(pool)` devolviendo el número de filas de cada tabla.

**Given** una base de datos con tablas pobladas
**When** se llama a `StatsRepo::db_sizes(pool)`
**Then** devuelve `Vec<TableSize>` con nombre de tabla y row count para:
events, llm_requests, memory, message_embeddings, messages, notes, profiles, reminders, settings, tasks, tools

#### Scenario: Tablas con datos
**Given** 10 messages, 2 profiles, 5 memories
**When** `StatsRepo::db_sizes(pool)`
**Then** messages=10, profiles=2, memories=5, resto=0
