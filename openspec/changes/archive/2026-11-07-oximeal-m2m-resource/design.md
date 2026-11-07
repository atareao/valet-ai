# Design

## Context

Ver `proposal.md` para la motivación. El sondeo real (`GET /api/oximeal/check` → `probe()` → `recent_recipes(1)`) obtiene un token M2M contra el `token_endpoint` descubierto por OIDC y luego llama a la Agent API de OxiMeal. Hoy el cuerpo de `client_credentials` lleva `grant_type`, `client_id`, `client_secret` y `scope`, pero no `resource`. La API de OxiMeal es `https://oximeal.territoriolinux.es/api`, que coincide con `<base_url>/api` con la base por defecto. El `detail` de los errores de fase de token ya pasa por `redact`, que borra el secreto.

## Decisions

1. **Derivar `<base_url>/api` en vez de exigir `oximeal_resource`.** La API de OxiMeal cuelga siempre de `<base_url>/api`, así que la instalación por defecto funciona sin configurar nada; el ajuste/entorno queda como override para despliegues donde la API viva en otro host. Alternativa descartada: exigir el ajuste → rompería la configuración por defecto y obligaría a un paso manual que el valor podría deducir solo.

2. **Precedencia ajuste → entorno → derivado, con barra final recortada.** Coherente con el resto de ajustes de OxiMeal (`resolve_base_url`, `resolve_issuer_url`, `resolve_client_id`). Alternativa descartada: solo entorno → obligaría a tocar el despliegue para un valor derivable de `base_url`. Recortar `/` evita generar `//api`.

3. **Exponer el `detail` redactado del IdP en `check_detail` en vez de mantener el mensaje opaco.** El síntoma «el IdP no entregó token» ocultaba precisamente la causa (`resource` ausente) y hacía indiagnosticable el fallo. El `detail` ya viene redactado por el servicio, así que nunca contiene el secreto. Alternativa descartada: mantener el texto opaco → el operador seguiría sin poder distinguir un `resource` mal formado de un `scope` no autorizado. El campo de detalle pasa conceptualmente de `&'static str` a `String`.

4. **`resource` solo en la petición M2M (`client_credentials`), no en el descubrimiento OIDC ni en las llamadas a la Agent API.** El documento `/.well-known/openid-configuration` no lo necesita y la Agent API se autoriza con el `Bearer`. Alternativa descartada: inyectarlo también en el descubrimiento → ruido sin efecto.

## Risks / Trade-offs

- Base con barra final → recortarla (`trim_end_matches('/')`) para que el `resource` derivado nunca sea `//api`.
- `OIDC_API_AUDIENCE` de OxiMeal distinta de `<base_url>/api` → el `resource` derivado no coincidiría y OxiMeal devolvería `401`; mitigación: override `oximeal_resource` y, ahora, el detalle del error visible en la sonda.
- Pocket ID sin la API registrada → seguirá rechazando la petición aunque se envíe `resource`; el nuevo detalle de la sonda lo hace visible en lugar de quedar oculto tras «el IdP no entregó token».
- Riesgo de fuga: el `detail` que se expone procede de `TokenEndpoint`/`Upstream`, que pasan por `redact`; nunca debe exponerse un detalle sin redactar.
