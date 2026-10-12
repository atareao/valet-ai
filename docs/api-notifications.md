# Notifications API

Valet exposes an HTTP API to push notifications into its inbox from any external
application and to read that inbox back. Ingestion is authenticated with a
per-application **ingest key** (a bearer token), while the inbox, the key
management and the WebPush transport use the **OIDC session**. With it you can:
send an alert from a backup script, build a client for the inbox, rotate
credentials, and let the browser receive the same alerts as WebPush
notifications.

## Endpoints at a glance

| Method | Route | Authentication | What it does |
|--------|-------|----------------|--------------|
| POST | `/api/notifications` | Ingest token (`Authorization: Bearer <token>`) | Ingest a notification; responds `201` with its id |
| GET | `/api/notifications/unread` | OIDC session | List unread notifications, newest first |
| POST | `/api/notifications/mark-read` | OIDC session | Idempotently mark notifications as read |
| POST | `/api/ingest-keys` | OIDC session | Create an ingest key and reveal its token once |
| GET | `/api/ingest-keys` | OIDC session | List keys (active and revoked), never a token |
| DELETE | `/api/ingest-keys/{id}` | OIDC session | Revoke a key, idempotently |
| GET | `/api/push/vapid-key` | OIDC session | Return the public VAPID key for the browser |
| POST | `/api/push/subscriptions` | OIDC session | Register (upsert) a browser push subscription |
| DELETE | `/api/push/subscriptions` | OIDC session | Unregister a push subscription by endpoint |

The only route exempt from the OIDC session is `POST /api/notifications`; it is
authenticated by an ingest token instead. Everything else under `/api/*`, and in
particular the three routes of `/api/ingest-keys` and `/api/push`, requires a
valid session cookie.

## Authentication

### Creating an ingest key

Ingest keys are created through the session-protected management route (see
[Keys](#keys)). The server generates the token, returns it **once**, and stores
only its SHA-256 hash plus the first 12 characters (the `prefix`).

### Token format

```
val_<base64url of 32 random bytes, no padding>
```

The literal prefix `val_` is followed by the base64url encoding (no `=` padding)
of 32 CSPRNG bytes. Every token starts with `val_`; the stored `prefix` is its
first 12 characters.

### Using the token

Send the token in the standard `Authorization` header:

```
Authorization: Bearer <your-token>
```

### Rules you must respect

- **The token is shown only once.** The `POST /api/ingest-keys` response is the
  only place it appears. Store it in your application's secret store right away;
  it cannot be retrieved later.
- **A missing header is a `401`.** If the `Authorization` header is absent, or
  the value does not start with `Bearer ` (with that exact spelling and a single
  space), the request is rejected with `401`.
- **Unknown and revoked tokens are indistinguishable.** Both answer with the same
  `401` body. The rejection reason (`missing_authorization`,
  `malformed_authorization`, `unknown_or_revoked_key`) goes to the server log
  only — never to the client.
- The token is never logged by the server. At most the key `id` and `name` are
  recorded.

## Ingesting a notification

`POST /api/notifications` is authenticated with an ingest token (see above) and
requires `Content-Type: application/json`.

### Body fields

| Field | Type | Required | Limit | Default |
|-------|------|----------|-------|---------|
| `message` | string | Yes | `≤ 4096` bytes after `trim`; must not be blank | — |
| `origin` | string | No | `≤ 64` bytes after `trim` | The name of the authenticating key |
| `author` | string | No | `≤ 128` bytes after `trim` | The name of the authenticating key |
| `level` | string | No | One of `info`, `warning`, `critical` | `info` |
| `metadata` | object | No | Serialized form `≤ 8192` bytes; must be a JSON **object** | absent |

A field that is missing, or blank after `trim`, falls back to its default
(`origin` and `author` to the key name, `level` to `info`). `level` is matched
exactly against the whitelist — it is **not** trimmed, so `" info "` is
rejected.

### Example request

```bash
curl -i -X POST http://localhost:3000/api/notifications \
  -H "Authorization: Bearer <your-token>" \
  -H "Content-Type: application/json" \
  -d '{
        "message": "Backup finished with 3 warnings",
        "origin": "backup-host",
        "author": "nightly-job",
        "level": "warning",
        "metadata": { "warnings": 3, "duration_s": 812 }
      }'
```

### Example response (`201 Created`)

```json
{
  "id": "3f2a1c7e-9b4d-4e6a-8f0c-1d2e3f4a5b6c"
}
```

The row is persisted **before** the response is emitted. WebPush delivery happens
afterwards, in the background, and is *best-effort*: a push failure is logged but
does not change the response you already received.

### Error responses

| Status | When | Body |
|--------|------|------|
| `400` | `message` missing or blank after `trim` | `{"error": "message is required"}` |
| `400` | `message` exceeds 4096 bytes | `{"error": "message must be at most 4096 bytes"}` |
| `400` | `origin` exceeds 64 bytes | `{"error": "origin must be at most 64 bytes"}` |
| `400` | `author` exceeds 128 bytes | `{"error": "author must be at most 128 bytes"}` |
| `400` | `level` not in the whitelist | `{"error": "level must be one of info, warning or critical"}` |
| `400` | `metadata` present but not an object | `{"error": "metadata must be a JSON object"}` |
| `400` | `metadata` serializes to more than 8192 bytes | `{"error": "metadata must serialize to at most 8192 bytes"}` |
| `401` | Header absent, malformed, or token unknown/revoked | `{"error": "unauthorized"}` |
| `413` | HTTP body larger than 64 KiB | (empty body; enforced by the body limit) |
| `415` | `Content-Type` missing or not `application/json` | (body produced by the JSON extractor) |
| `500` | Unexpected server error (e.g. database failure) | `{"error": "<message>"}` |

Example `401` body:

```json
{ "error": "unauthorized" }
```

Example `400` body:

```json
{ "error": "message is required" }
```

## The inbox

Both inbox routes require an OIDC session cookie. They are not reachable with an
ingest token.

### `GET /api/notifications/unread`

Lists unread notifications, newest first. `limit` is optional: it defaults to
`50` and must be an integer in `[1, 200]`. A non-numeric or out-of-range value
answers `400`. The view does not paginate beyond `limit` — it truncates, and
`next_cursor` is always `null`.

Example request:

```bash
curl -i "http://localhost:3000/api/notifications/unread?limit=20" \
  --cookie "valet_session=<your-session-cookie>"
```

Example response (`200 OK`):

```json
{
  "data": [
    {
      "id": "3f2a1c7e-9b4d-4e6a-8f0c-1d2e3f4a5b6c",
      "origin": "backup-host",
      "author": "nightly-job",
      "message": "Backup finished with 3 warnings",
      "level": "warning",
      "metadata": { "warnings": 3, "duration_s": 812 },
      "created_at": "2026-10-12 03:14:07"
    }
  ],
  "next_cursor": null,
  "total": 1
}
```

Each item exposes `id`, `origin`, `author`, `message`, `level`, `metadata` and
`created_at`. `metadata` arrives already parsed to a JSON object, or `null` when
the column is empty.

### `POST /api/notifications/mark-read`

Marks the given ids as read. It is idempotent: only rows that were still unread
are counted, repeating the call returns `0`, and ids that do not exist are
ignored without error. A missing or empty `ids` list answers `400`.

Example request:

```bash
curl -i -X POST http://localhost:3000/api/notifications/mark-read \
  --cookie "valet_session=<your-session-cookie>" \
  -H "Content-Type: application/json" \
  -d '{ "ids": ["3f2a1c7e-9b4d-4e6a-8f0c-1d2e3f4a5b6c"] }'
```

Example response (`200 OK`):

```json
{ "marked": 1 }
```

Example `400` body when `ids` is missing or empty:

```json
{ "error": "ids is required and must not be empty" }
```

## Keys

These three routes manage ingest keys and require an OIDC session. They never
return a token, except the creation response.

### `POST /api/ingest-keys`

Creates a key. `name` is required, must not be blank after `trim`, and must be
`≤ 64` bytes. A duplicate name (or a duplicate token hash) answers `409`.

Example request:

```bash
curl -i -X POST http://localhost:3000/api/ingest-keys \
  --cookie "valet_session=<your-session-cookie>" \
  -H "Content-Type: application/json" \
  -d '{ "name": "backup-host" }'
```

Example response (`201 Created`):

```json
{
  "id": "8a7b6c5d-4e3f-2a1b-0c9d-8e7f6a5b4c3d",
  "name": "backup-host",
  "prefix": "val_XXXXXXXX",
  "token": "val_XXXXXXXX<full-token-shown-only-once>"
}
```

> [!warning] The `token` is returned only in this response and never stored in
> clear. Copy it now; no later request can reveal it again.

### `GET /api/ingest-keys`

Lists all keys — active **and** revoked — newest first. It never includes a token
or a hash.

Example request:

```bash
curl -i http://localhost:3000/api/ingest-keys \
  --cookie "valet_session=<your-session-cookie>"
```

Example response (`200 OK`):

```json
[
  {
    "id": "8a7b6c5d-4e3f-2a1b-0c9d-8e7f6a5b4c3d",
    "name": "backup-host",
    "prefix": "val_XXXXXXXX",
    "created_at": "2026-10-12 09:00:00",
    "last_used_at": "2026-10-12 09:05:12",
    "revoked_at": null
  }
]
```

### `DELETE /api/ingest-keys/{id}`

Revokes a key by setting `revoked_at`; the row is kept. It answers `204` always,
whether or not the id exists. Revocation is idempotent: repeating it does not
change the already-set `revoked_at`. A revoked key no longer authenticates
ingestion (`401`).

Example request:

```bash
curl -i -X DELETE http://localhost:3000/api/ingest-keys/8a7b6c5d-4e3f-2a1b-0c9d-8e7f6a5b4c3d \
  --cookie "valet_session=<your-session-cookie>"
```

Example response: `204 No Content`, empty body.

## Push

The WebPush transport is driven by the browser, not by hand: the browser fetches
the public VAPID key, subscribes to its push service, and posts the resulting
subscription back. These three routes require an OIDC session; you normally do
not call them from your integration, only from the frontend.

### `GET /api/push/vapid-key`

Returns the public VAPID key in base64url (never the private one). If the keys
cannot be loaded or generated, it answers `503`.

Example request:

```bash
curl -i http://localhost:3000/api/push/vapid-key \
  --cookie "valet_session=<your-session-cookie>"
```

Example response (`200 OK`):

```json
{ "public_key": "<base64url-public-vapid-key>" }
```

Example `503` body:

```json
{ "error": "push_unavailable" }
```

### `POST /api/push/subscriptions`

Registers the browser's subscription. It is an **upsert by `endpoint`** that
keeps the original `id`; re-subscribing the same endpoint refreshes `p256dh`,
`auth` and `user_agent`. Validation failures answer `400`. `endpoint` must be an
absolute `https` URL, `p256dh` must decode (base64url, no padding) to 65 bytes
forming a valid uncompressed P-256 point, and `auth` must decode to 16 bytes. The
`User-Agent` header is stored with the subscription.

Example response (`201 Created`):

```json
{ "id": "1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d" }
```

### `DELETE /api/push/subscriptions`

Unregisters a subscription by `endpoint`. The body is required; a missing or
blank `endpoint` answers `400`. Otherwise it answers `204` always, whether or not
the row existed.

```json
{ "endpoint": "https://push.example.test/..." }
```

## Typical integration flow

1. **Create an ingest key** — call `POST /api/ingest-keys` with a session and a
   descriptive `name` (e.g. `backup-host`).
2. **Store the token immediately** — the response is the only time the `token`
   appears. Put it in your application's secret store.
3. **Send notifications** — call `POST /api/notifications` with
   `Authorization: Bearer <your-token>` whenever your application has something
   to report. Defaults fill `origin`/`author` with the key name.
4. **Rotate or revoke** — to decommission a producer, `DELETE
   /api/ingest-keys/{id}`; the token stops working immediately (`401`).
5. **Read the inbox** — a client with an OIDC session lists unread items via
   `GET /api/notifications/unread` and marks them read via
   `POST /api/notifications/mark-read`.
