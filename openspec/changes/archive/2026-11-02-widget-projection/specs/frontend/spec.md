## ADDED Requirements

### Requirement: El frontend SHALL componer el mensaje del asistente por tramos y colocar cada widget en su `span`

El frontend SHALL componer el mensaje del asistente como una **secuencia de tramos**: los tramos de texto del
`content` y, en el sitio del tramo que cada widget declara, el widget. Un widget **sin** `span` SHALL pintarse
**debajo** del texto íntegro, como hoy. La degradación de un `span` es **siempre de colocación, nunca de
contenido**: el contenido del mensaje SHALL conservarse íntegro —sin pérdida ni duplicación— y SHALL NOT
producirse ningún error de render.

La degradación es **por widget**: un widget cuyo `span` sea inválido —fuera del contenido, invertido, no entero
o que parta un carácter multibyte— o **solapado** con el de otro widget SHALL degradarse **él solo** (su
colocación cae al final, debajo del texto) sin afectar a los widgets válidos, que SHALL conservar cada uno su
tramo. Un contenido vacío SHALL NOT romper el render: no hay tramos de texto y los widgets se pintan debajo.

La composición SHALL residir en una **función pura** (`composeMessage(content, widgets)`) que devuelva los
tramos ordenados, con tests unitarios de los casos límite, de modo que la aritmética de tramos se verifique
sin montar un componente.

`useSSE` SHALL reconocer el campo `span` del evento `widget` y entregarlo a su callback; `useMainChat` SHALL
conservar ese `span` en el widget asociado al mensaje en curso, tanto durante el stream como al reconstruir
el historial desde `messages.widgets`. `useMainChat` SHALL, al recibir un evento `done` con `content`, fijar
el contenido del mensaje en curso a ese valor autoritativo —el texto íntegro que sirve de base a los
`span`—, sin perder los widgets ya asociados ni su identidad.

`MessageBubble` SHALL pintar los tramos en orden en lugar del texto seguido de los widgets.

#### Scenario: El widget se pinta en el sitio de la lista

**Given** un mensaje del asistente cuyo `content` contiene una lista de opciones y un widget con un `span` que la delimita
**When** se renderiza el mensaje
**Then** el texto anterior al `span` SHALL pintarse antes del widget
**And** el widget SHALL pintarse en el sitio del tramo
**And** el texto posterior al `span` SHALL pintarse después del widget
**And** la lista en prosa SHALL NOT pintarse

#### Scenario: Un widget sin tramo se pinta debajo

**Given** un mensaje del asistente con un widget sin `span` —un widget del modelo o una fila anterior a este change—
**When** se renderiza el mensaje
**Then** el texto SHALL pintarse íntegro
**And** el widget SHALL pintarse debajo del texto

#### Scenario: Un tramo inválido no rompe el render

**Given** un mensaje con varios widgets, uno de ellos con un `span` inválido (fuera del contenido, invertido, no entero, partiendo un carácter multibyte) o solapado con el de otro widget
**When** se renderiza el mensaje
**Then** el widget del `span` inválido o solapado SHALL degradarse al final, debajo del texto
**And** los widgets con `span` válido SHALL conservar cada uno su tramo
**And** el contenido SHALL pintarse íntegro, sin pérdida ni duplicación
**And** SHALL NOT producirse ningún error de render

#### Scenario: Un `done` con contenido fija el texto autoritativo

**Given** un stream en curso con texto acumulado y un widget con `span` ya asociado al mensaje
**When** llega un evento `done` con `content`
**Then** el contenido del mensaje SHALL quedar fijado al `content` del `done`
**And** el widget asociado SHALL conservarse con su `span`
