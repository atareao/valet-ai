# json-token-estimator Specification

## Purpose

Estimación determinista del número de tokens de una cadena JSON, sin dependencias, red ni I/O. La usa la Capa C para medir el estado persistente contra su presupuesto.

## Requirements

### Requirement: La estimación de tokens de un JSON SHALL ser determinista, propia y sin dependencias

El sistema SHALL exponer `estimate_json_tokens(json: &str) -> usize`, que estime el número de
tokens de una cadena JSON sin dependencias externas, sin red y sin I/O. La misma entrada SHALL
producir siempre el mismo resultado. La estimación SHALL basarse en la estructura del JSON y no
en un modelo lingüístico concreto, de modo que sirva igual para cualquier proveedor de LLM.

**Given** una cadena JSON  
**When** se estima su número de tokens  
**Then** el resultado SHALL ser determinista para la misma entrada  
**And** SHALL NOT requerir dependencias, red ni ficheros

#### Scenario: Resultado determinista
**Given** una cadena JSON concreta  
**When** se estima dos veces  
**Then** el resultado es el mismo

#### Scenario: Sin dependencias ni I/O
**Given** el estimador  
**When** se ejecuta sobre una entrada  
**Then** no descarga vocabularios, no accede a la red y no lee ni escribe ficheros

### Requirement: La estimación SHALL tratar la estructura del JSON

El estimador SHALL recorrer el JSON contando: los símbolos estructurales (`{`, `}`, `[`, `]`,
`:`, `,`), las cadenas (con una ratio de caracteres por token distinta para ASCII y para
no-ASCII), los literales (`true`, `false`, `null`), los números y los tramos de espacios
(colapsados). Los escapes dentro de las cadenas (`\"`, `\\`) SHALL tratarse como contenido y
NO como fin de cadena. Las ratios y constantes SHALL ser una aproximación determinista y
documentada, NO calibrada contra un tokenizer concreto.

**Given** un JSON con símbolos estructurales, cadenas, literales y números  
**When** se estima su número de tokens  
**Then** SHALL contar los símbolos estructurales  
**And** SHALL estimar las cadenas por su contenido  
**And** SHALL contar los literales `true`/`false`/`null` y los números  
**And** los escapes SHALL NOT cerrar la cadena

#### Scenario: Estructura contada
**Given** el JSON `{"a":1,"b":[true,null]}`  
**When** se estima  
**Then** se cuentan las llaves, los corchetes, los dos puntos y las comas

#### Scenario: Escapes no cierran la cadena
**Given** una cadena JSON con contenido `"dice \"hola\""`  
**When** se estima  
**Then** el contenido interior se estima como una sola cadena  
**And** la comilla escapada NO se confunde con el fin de la cadena

#### Scenario: Cadena vacía
**Given** el JSON `{"a":""}`  
**When** se estima  
**Then** la cadena vacía aporta únicamente el coste de sus comillas

#### Scenario: Los espacios se colapsan y se cuentan
**Given** un JSON con un tramo de espacios  
**When** se estima  
**Then** el tramo aporta al menos un token y no uno por carácter

### Requirement: La Capa C SHALL medir su tamaño con este estimador

`payload_token_count` SHALL estimar los tokens del estado persistente usando
`estimate_json_tokens` sobre el JSON minificado. La medición de los mensajes de chat NO SHALL
verse afectada.

**Given** un estado persistente  
**When** se mide contra el presupuesto  
**Then** SHALL usarse `estimate_json_tokens` sobre el JSON minificado  
**And** el heurístico de markdown SHALL NOT usarse para la Capa C

#### Scenario: La Capa C usa el estimador propio
**Given** un payload válido  
**When** se calcula `payload_token_count`  
**Then** coincide con `estimate_json_tokens` del JSON minificado

#### Scenario: Los mensajes de chat no cambian de medidor
**Given** un mensaje de chat  
**When** se estiman sus tokens  
**Then** sigue usándose el heurístico de markdown
