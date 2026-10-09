# Spec Delta: frontend

## ADDED Requirements

### Requirement: SettingsDialog SHALL organize the Generación roles in sub-tabs

Dentro de la pestaña «Generación», SettingsDialog SHALL mostrar un `Tabs` anidado con una sub-pestaña
por rol —**Chat**, **Colapso**, **Fichas** y **Consolidación**— siguiendo el patrón de la pestaña
«Prompts». La sub-pestaña «Consolidación» corresponde a las claves `GENERATION_SEMANTIC_*`. Cada
sub-pestaña SHALL contener los tres campos del rol (temperatura numérica, razonamiento por selector
y tokens máximos numéricos), manteniendo `name`/`label` iguales a la clave cruda. Las sub-pestañas
SHALL usar `forceRender` para que los doce campos permanezcan registrados en el formulario aunque su
sub-pestaña no esté activa, de modo que el guardado siga enviando las doce claves.

**Given** el SettingsDialog abierto en la pestaña "Generación"  
**When** se renderiza  
**Then** existe un `Tabs` anidado con las sub-pestañas Chat, Colapso, Fichas y Consolidación  
**And** la sub-pestaña activa muestra temperatura, razonamiento y tokens máximos de su rol

#### Scenario: Existen las cuatro sub-pestañas de rol
**Given** el SettingsDialog abierto en la pestaña "Generación"  
**When** se renderiza  
**Then** existen sub-pestañas con nombre Chat, Colapso, Fichas y Consolidación

#### Scenario: Cambiar de sub-pestaña muestra los campos del rol
**Given** el SettingsDialog abierto en "Generación" con la sub-pestaña "Chat" activa  
**When** el usuario selecciona la sub-pestaña "Colapso"  
**Then** los campos de colapso (`GENERATION_COLLAPSE_*`) quedan visibles  
**And** la sub-pestaña "Chat" deja de estar visible

#### Scenario: Los doce campos siguen registrados con forceRender
**Given** el usuario abrió la pestaña "Generación"  
**When** guarda sin haber abierto todas las sub-pestañas  
**Then** `updateSettings` recibe las doce claves `GENERATION_*` sin cambios en las no editadas

### Requirement: SettingsDialog SHALL fit all its top-level tabs without overflow

El `Modal` de SettingsDialog SHALL declarar un ancho de al menos **860 px** (se fija en 900) para que
sus siete pestañas superiores —Perfil, Interfaz, Prompts, API Keys, Memoria, Generación y Memoria
persistente— quepan en una sola fila, sin que ninguna se oculte en el desplegable de desbordamiento
de antd.

**Given** el SettingsDialog abierto  
**When** se renderiza  
**Then** el ancho del diálogo es de al menos 860 px  
**And** la pestaña "Memoria persistente" es alcanzable

#### Scenario: El diálogo es más ancho que el mínimo
**Given** el SettingsDialog abierto  
**When** se renderiza  
**Then** el ancho declarado del `Modal` es mayor o igual a 860 px

#### Scenario: La pestaña Memoria persistente es alcanzable
**Given** el SettingsDialog abierto  
**When** el usuario selecciona la pestaña "Memoria persistente"  
**Then** se muestra el panel de memoria persistente
