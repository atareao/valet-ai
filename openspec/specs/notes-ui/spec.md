# notes-ui Specification

## Purpose
La vista de Notas de la aplicación: ver, crear, editar y borrar las notas del perfil desde un modal
abierto desde la cabecera, con estados de carga, error y lista vacía, y soporte para las categorías
legadas.

## Requirements

### Requirement: La interfaz SHALL ofrecer una vista de Notas para verlas, crearlas, editarlas y borrarlas

La interfaz SHALL añadir una vista de **Notas**, abierta desde un botón de la cabecera de
`AppLayout` con `aria-label`/`title` «Notas», que se carga de forma diferida (`lazy`) y se muestra
como modal, siguiendo el patrón de `JournalView`. La vista SHALL listar las notas del perfil —con su
categoría, sus etiquetas y sus fechas— y SHALL permitir **crear**, **editar** y **borrar**:

- El listado SHALL pedir las notas al montar y **recargarse** tras cada mutación.
- El alta y la edición SHALL usar un formulario con el **contenido** (texto), la **categoría**
  (desplegable con `idea`, `journal` y `fact`, con etiquetas legibles) y las **etiquetas**
  (cadena separada por comas). El contenido SHALL ser obligatorio.
- El borrado SHALL pedir **confirmación** antes de ejecutarse.
- La vista SHALL mostrar estados de **carga**, **error de lectura** y **lista vacía**, y un fallo de
  mutación SHALL avisar sin cerrar el formulario.
- Las fechas SHALL formatearse **partiendo** la cadena ISO recibida, nunca pasándola por `new Date()`.

Las etiquetas SHALL viajar a la API como **cadena** (coma-separada). Una nota cuya categoría no sea
una de las tres (dato legado `todo`) SHALL mostrarse igualmente, pero guardarla SHALL obligar a elegir
una de las tres categorías.

#### Scenario: El botón abre la vista de Notas

- **Given** la aplicación autenticada
- **When** se pulsa el botón «Notas» de la cabecera
- **Then** se muestra la vista de Notas como modal

#### Scenario: La vista lista las notas del perfil

- **Given** un perfil con notas
- **When** se abre la vista de Notas
- **Then** se muestran las notas con su categoría, sus etiquetas y sus fechas

#### Scenario: Se crea una nota desde la vista

- **Given** el formulario de alta
- **When** se escribe un contenido, se elige una categoría y se guarda
- **Then** la nota se crea y el listado se recarga con la nota nueva

#### Scenario: Se edita una nota desde la vista

- **Given** una nota existente
- **When** se edita su contenido y se guarda
- **Then** la nota se actualiza y el listado se recarga con el contenido nuevo

#### Scenario: El borrado pide confirmación

- **Given** una nota existente
- **When** se solicita borrarla
- **Then** se pide confirmación antes de ejecutar el borrado

#### Scenario: Una nota con categoría legada obliga a elegir al guardar

- **Given** una nota cuya categoría es `todo` (dato legado)
- **When** se abre su edición y se guarda
- **Then** se exige elegir una de las tres categorías antes de guardar
