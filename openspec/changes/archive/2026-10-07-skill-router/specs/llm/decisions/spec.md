## Purpose

El cliente de la Decisions API de Jev: el contrato de la petición tipada, el tipado de las respuestas y el tratamiento de errores, aislado del endpoint en fase alpha y de la credencial del chat.

## ADDED Requirements

### Requirement: El cliente SHALL hablar la Decisions API con preguntas tipadas

El cliente SHALL enviar `POST` a `{ROUTER_BASE_URL}/alpha/decisions` con la credencial de OpenRouter en la cabecera de autorización y un cuerpo con el modelo configurado, el `state` textual y un objeto `questions`. En esta entrega cada pregunta SHALL ser de tipo `noul` e SHALL llevar sus instrucciones y sus `criteria` para el sí y para el no. El cliente SHALL aplicar su propio timeout, independiente del usado por el chat.

#### Scenario: La petición tiene la forma del servicio
- **Given** un estado y dos skills enrutables
- **When** el cliente construye y envía la petición
- **Then** el cuerpo lleva el modelo configurado, el estado y una pregunta `noul` por skill
- **And** cada pregunta incluye instrucciones y las dos `criteria`

#### Scenario: El modelo es el configurado
- **Given** un modelo distinto del default en los ajustes
- **When** se envía la petición
- **Then** el cuerpo declara ese modelo

### Requirement: La respuesta SHALL tiparse y el coste SHALL exponerse

El cliente SHALL leer una probabilidad `noul` por cada pregunta respondida y SHALL exponer los tokens de entrada y salida y el coste que devuelve el servicio. Los identificadores de pregunta desconocidos SHALL ignorarse y una respuesta sin objeto de respuestas SHALL considerarse error.

#### Scenario: Se leen las probabilidades y el coste
- **Given** una respuesta con dos preguntas y su bloque de uso
- **When** el cliente la interpreta
- **Then** devuelve la probabilidad de cada pregunta
- **And** expone los tokens y el coste

#### Scenario: Una respuesta sin respuestas es un error
- **Given** una respuesta válida a nivel de JSON pero sin objeto de respuestas
- **When** el cliente la interpreta
- **Then** devuelve un error

#### Scenario: Un identificador desconocido se ignora
- **Given** una respuesta que incluye una pregunta no solicitada
- **When** el cliente la interpreta
- **Then** ese identificador se ignora sin error

### Requirement: Los fallos del servicio SHALL ser errores tipados, nunca pánicos

Un código de estado no exitoso, un timeout, un JSON inválido o una probabilidad fuera del rango `[0, 1]` SHALL producir un error tipado que el enrutador pueda tratar como fallo abierto.

#### Scenario: Un error del servicio se propaga como error
- **Given** un servicio que responde con error de servidor
- **When** el cliente envía la petición
- **Then** devuelve un error tipado
- **And** no produce pánico

#### Scenario: Una probabilidad fuera de rango es un error
- **Given** una respuesta con una probabilidad mayor que uno
- **When** el cliente la interpreta
- **Then** devuelve un error tipado

### Requirement: El cliente SHALL quedar tras un trait inyectable

El enrutador SHALL depender de un trait de clasificación, no del cliente concreto, de modo que las pruebas puedan inyectar un doble determinista sin red.

#### Scenario: El orquestador no conoce el cliente concreto
- **Given** un orquestador en pruebas
- **When** se le inyecta un doble del clasificador
- **Then** el turno enruta según el doble
- **And** no se realiza ninguna petición de red
