# Spec Delta: orchestrator/agent

## ADDED Requirements

### Requirement: La petición al LLM SHALL ofrecer únicamente las herramientas habilitadas

Cada `ChatRequest` construida en el bucle ReAct SHALL incluir en `tools` únicamente las definiciones
de las herramientas habilitadas en ese momento.

#### Scenario: Una herramienta deshabilitada queda fuera de la petición

**Given** un orquestador con `unified_search` deshabilitada
**When** construye la petición al LLM
**Then** `request.tools` NO contiene la definición de `unified_search`
**And** sí contiene las definiciones de las herramientas habilitadas
