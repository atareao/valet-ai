## ADDED Requirements

### Requirement: ChatResponse SHALL expose the finish reason

`ChatResponse` SHALL expose the completion's finish reason as `finish_reason: Option<FinishReason>`,
where `FinishReason` is a typed enum with at least `Stop`, `Length` and `Other(String)`, so callers
can tell a truncated completion apart from a complete one. It SHALL be `None` when the provider does
not report one (e.g. Ollama). Both the non-streaming and the streaming paths SHALL populate it when
the provider reports it.

#### Scenario: A truncated completion is reported as Length
**Given** an OpenRouter response whose `choices[0].finish_reason` is `"length"`
**When** the provider parses it
**Then** `finish_reason` is `Some(FinishReason::Length)`

#### Scenario: An absent finish reason is None
**Given** a provider response without `finish_reason`
**When** the provider parses it
**Then** `finish_reason` is `None`

#### Scenario: Ollama does not report a finish reason
**Given** an Ollama response
**When** the provider parses it
**Then** `finish_reason` is `None`
