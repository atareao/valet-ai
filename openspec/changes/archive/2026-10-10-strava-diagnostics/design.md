# Design: Diagnóstico de la conexión con Strava

## El cuerpo del error de Strava

Strava responde con un JSON estable en sus fallos:

```json
{
  "message": "Forbidden",
  "errors": [{ "resource": "Application", "field": "Status", "code": "Inactive" }]
}
```

`errors` puede venir vacío (`{"message":"Authorization Error","errors":[]}`) o ausente. El parseo
SHALL ser tolerante: si el cuerpo no es JSON o no encaja, se conserva el **código HTTP** y, si acaso, un
recorte del cuerpo. Nunca un pánico.

### Mapeo (decidido)

| Respuesta de Strava | `StravaError` | Mensaje al usuario |
|:--|:--|:--|
| `401` | (flujo actual: refresco + un reintento) | `error de Strava: …` si persiste |
| `429` | `RateLimited` | límite de tasa alcanzado |
| `403` con `Application/Status/Inactive` | **`ApplicationInactive`** | la app de Strava está inactiva; reactívala en `https://www.strava.com/settings/api` (la cuenta propietaria necesita suscripción activa) |
| otro `4xx`/`5xx` | `Http(...)` | `<status> · <message> · <field>:<code>` |
| cuerpo no JSON / vacío | `Http(...)` | `<status>` (sin inventar contenido) |

El mismo parseo se aplica a los fallos del canje y del refresco de tokens (`POST /oauth/token`).

## `GET /api/strava/check` — por qué `200` siempre

La comprobación existe para **informar**, no para fallar: `ok: false` con un mensaje accionable es un
resultado legítimo del sondeo. Devolver `500` obligaría a la interfaz a distinguir «fallo del sondeo» de
«sondeo que reporta un fallo», que es justo la confusión que este cambio elimina.

- `GET /api/strava/check` → `200` con `{ "ok": bool, "athlete_id": string|null, "athlete_name":
  string|null, "error": string|null }`.
- Consulta `GET /api/v3/athlete` **sin servirse de la caché corta** (60 s): una comprobación pedida a
  propósito SHALL reflejar el estado de ahora.
- Sin conexión o sin credenciales → `200` con `ok: false` y el mismo mensaje accionable que usan las
  herramientas.
- No consume presupuesto de enrutado: no es una herramienta, no entra en el catálogo y no la ve el
  modelo.

## Desconectar: limpiar siempre, avisar si la revocación falla

`disconnect` limpia las claves locales **siempre** (la intención del usuario es desconectar) y comprueba
la respuesta de `POST /oauth/revoke`:

- Revocación confirmada (`2xx`) → `{ "connected": false }`.
- Revocación no confirmada (error de red, `401` por credenciales, `5xx`…) → `{ "connected": false,
  "warning": "…" }`, con el motivo y la recomendación de retirar el acceso desde
  `https://www.strava.com/settings/apps`.

Nunca se revierte la limpieza local por un fallo de Strava, y nunca se calla el fallo.
