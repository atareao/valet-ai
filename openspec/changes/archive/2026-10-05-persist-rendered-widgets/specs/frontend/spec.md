# Spec Delta: frontend

## ADDED Requirements

### Requirement: El frontend SHALL reconstruir los widgets persistidos al cargar el historial

Al inicializar el chat (`GET /api/chat/init`), el frontend SHALL poblar `widgetsByMessage` a partir del
campo `widgets` de cada mensaje del asistente, de modo que los widgets renderizados en la conversación
vuelvan a mostrarse tras recargar la página. Los mensajes sin `widgets` no SHALL crear entradas.

#### Scenario: Los widgets reaparecen al recargar

**Given** un historial con un mensaje assistant que tiene `widgets = [{ id: "w1", name: "LocationWidget", data: {…} }]`
**When** el frontend inicializa el chat
**Then** `widgetsByMessage[<id del mensaje>]` contiene ese widget
**And** se renderiza con `WidgetRenderer`

#### Scenario: Mensaje sin widgets no crea entrada

**Given** un historial con un mensaje assistant sin `widgets`
**When** el frontend inicializa el chat
**Then** no se crea ninguna entrada en `widgetsByMessage` para ese mensaje
