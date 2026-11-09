# oximeal-api — Delta de especificación (change oximeal-recipe-instructions)

## ADDED Requirements

### Requirement: Una migración SHALL actualizar `SKILL_MENUS_PROMPT` para pedir las instrucciones, con guarda byte a byte

Una migración nueva, aditiva e idempotente SHALL actualizar el valor de `SKILL_MENUS_PROMPT` para que pida las instrucciones de elaboración al guardar recetas, conservando el encabezado `# SKILL ACTIVA: MENUS`. La migración SHALL sustituir **solo** los textos conocidos de la versión anterior del fragmento (guarda **byte a byte**): si el valor guardado no coincide con ninguno de los textos reconocidos —porque el usuario lo editó—, la migración SHALL NOT pisarlo. No SHALL tocar ningún otro ajuste.

#### Scenario: El fragmento por defecto se actualiza
- **Given** un `SKILL_MENUS_PROMPT` con el texto por defecto de la versión anterior
- **When** se aplica la migración
- **Then** el valor pasa a ser el texto que pide las instrucciones
- **And** conserva el encabezado `# SKILL ACTIVA: MENUS`

#### Scenario: Un fragmento editado por el usuario no se pisa
- **Given** un `SKILL_MENUS_PROMPT` con un texto distinto al por defecto (editado por el usuario)
- **When** se aplica la migración
- **Then** el valor no cambia

#### Scenario: La migración es idempotente
- **Given** la migración ya aplicada
- **When** se vuelve a ejecutar sobre el mismo valor
- **Then** el valor resulta igual al que dejó la primera ejecución
