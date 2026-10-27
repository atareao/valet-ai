# zennotes-api Specification

## Purpose
Los ajustes de la integración ZenNotes y su superficie HTTP: las claves en `settings`, la sensibilidad del token y los endpoints dedicados que permiten fijarlo sin exponerlo.

## Requirements

### Requirement: El token de ZenNotes SHALL ser un ajuste sensible con endpoints dedicados

`zennotes_token` SHALL formar parte de `SENSITIVE_KEYS`, de modo que NO aparezca en `GET /api/settings`
y SHALL ignorarse en `PUT /api/settings`. Como esa vía no puede escribirlos, la API SHALL exponer
`GET /api/zennotes/status` → `200 {"configured": <bool>, "base_url": <string>, "folder": <string>}`
**sin** el token, y `PUT /api/zennotes/token` con cuerpo `{"token": <string>}` que SHALL responder `400`
con un token ausente o vacío y, con un token válido, SHALL guardarlo en `settings` y responder sin
devolverlo. Los ajustes `zennotes_base_url` (por defecto `https://zennotes.territoriolinux.es`) y
`zennotes_folder` (por defecto `Valet`) SHALL NO ser sensibles.

#### Scenario: El token no aparece en la lectura de ajustes
- **Given** `zennotes_token` configurado
- **When** se hace `GET /api/settings`
- **Then** la respuesta NO contiene `zennotes_token`

#### Scenario: El estado informa sin revelar el token
- **Given** `zennotes_token` configurado
- **When** se hace `GET /api/zennotes/status`
- **Then** la respuesta indica `configured: true` con `base_url` y `folder`
- **And** NO incluye el token

#### Scenario: Un token vacío se rechaza
- **Given** una llamada `PUT /api/zennotes/token` con `{"token": ""}`
- **When** se ejecuta
- **Then** la respuesta es `400`

#### Scenario: Un token válido se guarda
- **Given** una llamada `PUT /api/zennotes/token` con un token no vacío
- **When** se ejecuta
- **Then** el token queda guardado en `settings`
- **And** la respuesta NO devuelve el token

### Requirement: Una migración SHALL sembrar los ajustes de ZenNotes y el fragmento de prompt

Una migración aditiva e idempotente SHALL sembrar `zennotes_base_url`
(`https://zennotes.territoriolinux.es`), `zennotes_folder` (`Valet`), `zennotes_token` (vacío),
`ROUTER_SKILL_ZENNOTES_ENABLED` (`true`) y el fragmento `SKILL_ZENNOTES_PROMPT`, sin pisar valores ya
fijados por el usuario.

#### Scenario: Los ajustes se siembran con sus defaults
- **Given** la base de datos recién migrada
- **When** se consultan los ajustes
- **Then** `zennotes_base_url` vale `https://zennotes.territoriolinux.es`
- **And** `zennotes_folder` vale `Valet`
- **And** `ROUTER_SKILL_ZENNOTES_ENABLED` vale `true`
- **And** `SKILL_ZENNOTES_PROMPT` no está vacío

#### Scenario: La migración respeta un valor ya fijado
- **Given** `settings.zennotes_folder = "MiVault"` antes de migrar
- **When** se ejecuta la migración
- **Then** el valor sigue siendo `MiVault`
