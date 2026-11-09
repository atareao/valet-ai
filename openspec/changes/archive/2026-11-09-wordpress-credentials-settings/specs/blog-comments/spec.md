# blog-comments — Delta de especificación (change wordpress-credentials-settings)

## REMOVED Requirements

### Requirement: Las credenciales SHALL resolverse desde el entorno y SHALL NOT filtrarse

## ADDED Requirements

### Requirement: Las credenciales SHALL resolverse desde los ajustes y SHALL NOT filtrarse

La herramienta SHALL resolver la URL base, el usuario de servicio y la Application Password desde los
ajustes `blog_base_url`, `blog_service_user` y `blog_app_password` en **cada llamada**, de modo que un
cambio en Ajustes no exija reiniciar; SHALL NOT leerlos de variables de entorno. `blog_base_url` SHALL
tener como valor por defecto `https://atareao.es`; `blog_service_user` y `blog_app_password` SHALL NOT
tener ningún valor por defecto. Sin usuario o sin contraseña la herramienta SHALL NOT enviar ninguna
petición y SHALL devolver un error accionable que nombre los ajustes que faltan y remita a
Ajustes → Integraciones → Blog, **sin** provocar un `panic!`. La contraseña de aplicación SHALL NOT
aparecer en ningún registro, ni en la telemetría, ni en el resultado de la herramienta, ni en un
mensaje de error, ni en la representación `Debug` de la configuración, que SHALL redactarla; la
cabecera `Authorization` SHALL construirse en el momento de la petición y SHALL NOT guardarse ni
registrarse.

#### Scenario: Sin credenciales no hay ninguna petición

- **Given** unos ajustes sin `blog_service_user` y sin `blog_app_password`
- **When** se ejecuta `blog_comment_reply` con argumentos válidos
- **Then** devuelve un error accionable que nombra los ajustes que faltan y remite a Ajustes → Integraciones → Blog
- **And** el servidor de prueba no recibe ninguna petición

#### Scenario: La URL base tiene un valor por defecto

- **Given** unos ajustes sin `blog_base_url`
- **When** se resuelve la configuración
- **Then** la URL base es `https://atareao.es`

#### Scenario: Las credenciales se leen de los ajustes y no del entorno

- **Given** unas variables de entorno `BLOG_SERVICE_USER` y `BLOG_APP_PASSWORD` definidas
- **And** unos ajustes sin `blog_service_user` ni `blog_app_password`
- **When** se ejecuta `blog_comment_reply` con argumentos válidos
- **Then** la herramienta falla por falta de credenciales en los ajustes
- **And** el servidor de prueba no recibe ninguna petición

#### Scenario: La contraseña no aparece en la representación de depuración

- **Given** una configuración con una contraseña de aplicación conocida
- **When** se formatea su `Debug`
- **Then** la contraseña NO aparece
- **And** la URL base y el usuario sí aparecen

#### Scenario: La contraseña no aparece en un mensaje de error

- **Given** un servidor de prueba que responde `401` con un cuerpo arbitrario
- **When** se ejecuta `blog_comment_reply`
- **Then** el mensaje del error NO contiene la contraseña de aplicación
- **And** el mensaje NO contiene la cabecera `Authorization`
