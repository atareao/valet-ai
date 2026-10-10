## ADDED Requirements

### Requirement: OpenRouterProvider SHALL reject a response body without choices

A response with an HTTP success status whose body carries no `choices` array (e.g. `{"error": …}`
reported by the upstream provider) SHALL be an `LLMError` — including the provider's message when
the body carries one — and SHALL NEVER become a successful `ChatResponse` with empty content.

#### Scenario: An error body on a 200 is an error
**Given** an HTTP 200 response whose body is `{"error":{"message":"Provider returned error"}}`
**When** the provider parses it
**Then** the call fails with an `LLMError`
**And** the error message contains `Provider returned error`

#### Scenario: A well-formed body still succeeds
**Given** an HTTP 200 response with `choices[0].message.content`
**When** the provider parses it
**Then** the call returns `Ok` with that content
