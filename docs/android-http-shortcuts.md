# Valet en «Atajos HTTP» de Android

Guía para lanzar peticiones a Valet desde la app Android **HTTP Shortcuts** («Atajos HTTP»): un
atajo de pantalla de inicio o un azulejo de ajustes rápidos que mete un aviso en la bandeja de
Valet sin abrir el navegador.

El contrato completo de la API está en [`api-notifications.md`](api-notifications.md); aquí
solo se documenta la parte que necesita el atajo y las decisiones que se tomaron al escribirlo.

## Qué se puede hacer y qué no

| Endpoint | Autenticación | ¿Sirve desde un atajo? |
|:--|:--|:--|
| `POST /api/notifications` | clave de ingesta (`Authorization: Bearer val_…`) | Sí, es el caso pensado |
| `GET /api/notifications/unread` y `POST /api/notifications/mark-read` | sesión OIDC (cookie) | Solo con cookie, y caduca |
| Resto de `/api/*` (chat, diario, eventos, ajustes…) | sesión OIDC | No |

La única ruta exenta de sesión es `POST /api/notifications`. Todo lo demás exige cookie de sesión,
que en Valet tiene un TTL fijo de 8 h (`plans/PLAN-010-PENDING.md`), así que no es una base sólida
para un atajo permanente.

## 1. Crear la clave de ingesta

En Valet, **Ajustes → pestaña «Claves» → «Claves de ingesta»**: pon un nombre descriptivo (por
ejemplo `movil-android`) y crea la clave.

El token se muestra **una sola vez** (empieza por `val_`); el servidor guarda solo su SHA-256 y el
prefijo de 12 caracteres. Cópialo en ese momento: no hay forma de recuperarlo después. Para
revocarlo, borra la clave en la misma sección; el token deja de autenticar de inmediato (`401`).

## 2. Construir el atajo

- **Método:** `POST`
- **URL:** `https://valet.territoriolinux.es/api/notifications`
- **Cabeceras:**
  - `Authorization: Bearer val_…`
  - `Content-Type: application/json`
- **Cuerpo (JSON):**

  ```json
  {
    "message": "{{texto}}",
    "origin": "movil-android",
    "level": "info"
  }
  ```

`message` es obligatorio (máximo 4096 bytes tras `trim`, no puede quedar en blanco). `origin` y
`author` son opcionales y, si se omiten, toman el nombre de la clave. `level` admite exactamente
`info`, `warning` o `critical`; no se recorta, así que `" info "` se rechaza.

### La variable del mensaje

Declara una variable de tipo texto llamada `texto` y configúrala para que el atajo **pregunte su
valor** al ejecutarse. Inserta la variable desde el selector del editor del cuerpo en lugar de
escribir la sintaxis a mano: cambia entre versiones de la app.

## 3. Llevarlo al acceso directo

- Mantén pulsado el atajo en la lista y elige **«Añadir a pantalla de inicio»**.
- O actívalo como **azulejo de ajustes rápidos** en los ajustes del atajo, para tenerlo en la
  cortina de notificaciones.

## Probar la credencial antes

Un `curl` desde el ordenador confirma la clave y el formato del cuerpo antes de pelearse con el
móvil:

```bash
curl -i -X POST https://valet.territoriolinux.es/api/notifications \
  -H "Authorization: Bearer val_..." \
  -H "Content-Type: application/json" \
  -d '{"message":"prueba desde curl","origin":"portatil","level":"info"}'
```

Respuesta esperada: `201` con `{"id": "…"}`.

## Respuestas y errores

| Código | Cuándo |
|:--|:--|
| `201` | Aviso persistido; el `id` viaja en el cuerpo |
| `400` | `message` ausente o en blanco, `level` fuera de la lista o `metadata` que no es objeto |
| `401` | Cabecera ausente o mal formada, o clave desconocida/revocada |
| `413` | Cuerpo mayor de 64 KiB |
| `415` | `Content-Type` ausente o distinto de `application/json` |

El `401` es deliberadamente indistinguible entre «clave desconocida» y «clave revocada»; el motivo
real solo aparece en el log del servidor.

## Límites y comportamiento

- `message`: 4096 bytes tras `trim`. `origin`: 64. `author`: 128. `metadata`: 8192 serializado.
- El aviso se persiste **antes** de responder. El WebPush al navegador es *best-effort*: si falla,
  se registra y tu `201` no cambia.
- No hay reintentos ni idempotencia: un corte de red significa que el aviso no entró.

## Seguridad

- Marca el token como **secreto** en las variables de Atajos HTTP para que no se muestre al editar
  el atajo.
- El token viaja en la cabecera `Authorization` sobre HTTPS; nunca en la URL, donde quedaría en los
  logs de proxies y servidores.
- Si el móvil se pierde o cambia de dueño, revoca la clave: es el único mecanismo de corte.

## Seguimiento

Leer la bandeja o mandar una frase al agente desde el móvil exigiría un endpoint autenticado por
clave (un «token de chat»), que hoy no existe. No está planificado: si aparece la necesidad, hay
que abrir un change con su spec, porque cambia el modelo de autenticación de la superficie de chat.