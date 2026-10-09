# consolidator-prompt-ui Specification

## ADDED Requirements

### Requirement: El prompt del consolidador SHALL poder verse y editarse desde la interfaz

La pestaña «Prompts» del diálogo de ajustes SHALL incluir una sub-pestaña «Consolidator» que
muestre el valor vigente de `settings.consolidator_prompt` en un área de texto editable, junto a
las sub-pestañas existentes *System*, *Archivist* y *Collapse*. El valor SHALL persistirse por el
endpoint de settings ya existente. Guardar el prompt SHALL NOT alterar el estado persistente ni el
presupuesto de tokens. La sub-pestaña SHALL advertir —sin bloquear el guardado— cuando el texto no
contenga los placeholders `{{ ESTADO_ACTUAL }}` o `{{ BLOQUE_DE_MENSAJES }}`, indicando cuáles
faltan.

**Given** el diálogo de ajustes abierto  
**When** se abre la pestaña «Prompts»  
**Then** SHALL existir una sub-pestaña «Consolidator»  
**And** su área de texto SHALL contener el valor vigente de `settings.consolidator_prompt`

#### Scenario: El prompt vigente se muestra al abrir la sub-pestaña

**Given** `settings.consolidator_prompt` con un valor que contiene ambos placeholders  
**When** se abre la sub-pestaña «Consolidator»  
**Then** el área de texto muestra ese valor  
**And** no se muestra ninguna advertencia de placeholders

#### Scenario: Editar y guardar el prompt no toca el resto de ajustes

**Given** un texto editado en la sub-pestaña «Consolidator»  
**When** se guarda  
**Then** SHALL enviarse `consolidator_prompt` con el texto editado  
**And** el resto de claves de settings SHALL conservarse  
**And** el estado persistente y el presupuesto de tokens SHALL permanecer intactos

#### Scenario: Aviso por placeholder ausente sin bloquear

**Given** un texto al que le falta `{{ BLOQUE_DE_MENSAJES }}`  
**When** se intenta guardar  
**Then** SHALL mostrarse una advertencia que nombra el placeholder ausente  
**And** el guardado SHALL proceder igualmente

#### Scenario: Texto con ambos placeholders no avisa

**Given** un texto que contiene `{{ ESTADO_ACTUAL }}` y `{{ BLOQUE_DE_MENSAJES }}`  
**When** se guarda  
**Then** NO SHALL mostrarse advertencia de placeholders  
**And** el valor SHALL persistirse
