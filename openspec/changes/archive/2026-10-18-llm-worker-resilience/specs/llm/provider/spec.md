## ADDED Requirements

### Requirement: ChatResponse SHALL expose the finish reason

`ChatResponse` SHALL expose the completion's finish reason as `finish_reason: Option<FinishReason>`,
where `FinishReason` is a typed enum with at least `Stop`, `Length` and `Other(String)`, so callers
can tell a truncated completion apart from a complete one. It SHALL be `None` when the provider does
not report one (e.g. Ollama). Both the non-streaming and the streaming paths SHALL populate it when
the provider reports it. On the wire it SHALL be the **flat** provider string (`"stop"`, `"length"`,
`"content_filter"`, …) and never a tagged object, so serialization and deserialization stay
symmetric with what the providers actually send; a value the enum does not recognise SHALL be kept
**verbatim** in `Other(String)` rather than rejected or lost.

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

#### Scenario: A finish reason travels as a flat string on the wire
**Given** a `FinishReason::Stop`
**When** it is serialized to the provider's JSON form
**Then** the value is the flat string `"stop"`, not a tagged object such as `{"stop": …}`
**And** deserializing that flat string yields `FinishReason::Stop` again

#### Scenario: An unrecognised finish reason is kept verbatim
**Given** an OpenRouter response whose `choices[0].finish_reason` is `"content_filter"`
**When** the provider parses it
**Then** `finish_reason` is `Some(FinishReason::Other("content_filter"))`
**And** serializing it back yields the same flat string `"content_filter"`
