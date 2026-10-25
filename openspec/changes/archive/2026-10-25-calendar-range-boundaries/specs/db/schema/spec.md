## MODIFIED Requirements

### Requirement: Expand events list_by_date_range to support recurring events

`EventsRepo::list_by_date_range` SHALL expandir los eventos recurrentes (con `rrule` no nulo)
generando las instancias que **se solapen** con el rango consultado. `expand_recurring` SHALL recibir
los extremos base y los límites del rango como **instantes ya parseados** (`DateTime<FixedOffset>`),
de modo que un límite de fecha suelta (`YYYY-MM-DD`) también funcione. Una instancia SHALL incluirse
cuando su intervalo solape el rango (`instancia_start < range_end AND instancia_end > range_start`).
Si un extremo base no es parseable, la expansión SHALL devolver una lista vacía. Las instancias SHALL
emitirse en UTC canónico (RFC 3339 con sufijo `Z`), convertidas desde el instante real del evento
base: una base almacenada con offset (`+02:00`) NO SHALL reetiquetarse con su hora de pared.

**Contracts:**

```rust
// Se modifica EventsRepo::list_by_date_range para expandir recurrencias.
// Por cada evento con rrule, se generan las instancias que solapan el rango.
// La expansion cubre: FREQ=DAILY, FREQ=WEEKLY, FREQ=MONTHLY
// Año y límite de expansión: máximo 365 instancias por evento.

fn expand_recurring(
    base_start: &DateTime<FixedOffset>,
    base_end: &DateTime<FixedOffset>,
    rrule: &str,
    range_start: &DateTime<FixedOffset>,
    range_end: &DateTime<FixedOffset>,
) -> Vec<(String, String)>;
```

**Scenarios:**

#### Scenario: List recurring event within range
Given an event "Standup" with rrule "FREQ=WEEKLY;BYDAY=MO,WE,FR" starting Monday
When `list_by_date_range` is queried for a week containing that Monday
Then the event appears 3 times (Mon, Wed, Fri)

#### Scenario: Recurring event outside range returns nothing
Given an event with rrule "FREQ=DAILY" but base dates are a year ago
When `list_by_date_range` is queried for next week
Then the event does NOT appear in results

#### Scenario: Non-recurring event still listed normally
Given a non-recurring event on Tuesday
When `list_by_date_range` is queried for that week
Then the event appears once

#### Scenario: Límites de fecha suelta
**Given** un evento "Standup" con rrule "FREQ=WEEKLY;BYDAY=MO,WE,FR" que empieza un lunes
**When** `list_by_date_range` se consulta con `start = "2026-09-28"` y `end = "2026-10-04"`
**Then** el evento SHALL aparecer 3 veces (lunes, miércoles y viernes)

#### Scenario: Instancia que solapa parcialmente el rango
**Given** una instancia recurrente que empieza antes del límite inferior del rango y termina dentro de él
**When** `list_by_date_range` se consulta con ese rango
**Then** la instancia SHALL incluirse

#### Scenario: Las instancias se emiten en UTC canónico
**Given** un evento con rrule "FREQ=DAILY" cuyo start_time es "2026-10-06T19:00:00+02:00" (17:00Z) y end_time "2026-10-06T20:00:00+02:00" (18:00Z)
**When** `expand_recurring` expande un rango que contiene el 2026-10-06
**Then** la instancia de ese día SHALL tener start_time="2026-10-06T17:00:00Z" y end_time="2026-10-06T18:00:00Z"

## ADDED Requirements

### Requirement: EventsRepo::list_by_date_range SHALL resolver el rango por instante y por solape

`EventsRepo::list_by_date_range` SHALL recibir los límites del rango como instantes
(`DateTime<FixedOffset>`) y SHALL devolver los eventos cuyo intervalo **se solape** con el rango
consultado, entendido como el intervalo semiabierto `[start, end)`
(`start_time < range_end AND end_time > range_start`). El filtro SHALL resolverse comparando
**instantes**, nunca el orden lexicográfico de las cadenas almacenadas, de modo que dos filas que
representan el mismo instante con offsets distintos se traten igual. El resultado SHALL ordenarse por
instante de inicio; una fila cuyos timestamps no se puedan parsear SHALL incluirse y SHALL quedar al
final del orden (falla abierto). `EventsRepo::list_by_category` SHALL ordenar con el mismo criterio.
Un helper público SHALL traducir un límite en texto a un instante: RFC 3339 (con offset o sufijo `Z`)
o fecha suelta `YYYY-MM-DD`, que SHALL cubrir el día UTC completo (`T00:00:00Z` como límite inferior y
el inicio del día siguiente como límite superior).

**Contracts:**

```rust
/// Traduce un límite de rango en texto a un instante.
/// - RFC 3339 con offset o sufijo `Z` → el instante tal cual.
/// - `YYYY-MM-DD` → día UTC completo: `T00:00:00Z`, o el inicio del día
///   siguiente cuando `upper = true`.
/// - Cualquier otro valor → `None` (límite no parseable).
pub fn parse_range_bound(raw: &str, upper: bool) -> Option<DateTime<FixedOffset>>;

impl EventsRepo {
    /// Intervalo semiabierto [start, end).
    /// Solape: start_time < end && end_time > start, comparando instantes.
    pub async fn list_by_date_range(
        pool: &SqlitePool,
        profile_id: &str,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> Result<Vec<Event>, sqlx::Error>;

    pub async fn list_by_category(
        pool: &SqlitePool,
        profile_id: &str,
        category: &str,
    ) -> Result<Vec<Event>, sqlx::Error>;
}
```

**Scenarios:**

#### Scenario: Un límite de fecha suelta incluye el último día
**Given** un evento con start_time="2026-10-31T08:00:00Z" y end_time="2026-10-31T09:00:00Z"
**When** `list_by_date_range` se consulta con `start = "2026-09-27"` y `end = "2026-10-31"`
**Then** el evento SHALL aparecer en el resultado

#### Scenario: Solape por delante
**Given** un evento con start_time="2026-09-26T23:00:00Z" y end_time="2026-09-27T01:00:00Z"
**When** `list_by_date_range` se consulta con `start = "2026-09-27T00:00:00Z"` y `end = "2026-09-28T00:00:00Z"`
**Then** el evento SHALL aparecer en el resultado

#### Scenario: Un evento que termina en el límite inferior queda fuera
**Given** un evento cuyo end_time es exactamente el límite inferior del rango consultado
**When** `list_by_date_range` se consulta con ese rango
**Then** el evento NO SHALL aparecer (el intervalo es semiabierto)

#### Scenario: Los formatos mezclados se comparan por instante
**Given** un evento A con start_time="2026-10-06T19:00:00+02:00" y end_time="2026-10-06T20:00:00+02:00" (17:00–18:00Z)
**And** un evento B con start_time="2026-10-06T18:30:00Z" y end_time="2026-10-06T19:30:00Z"
**When** `list_by_date_range` se consulta con `start = "2026-10-06T18:15:00Z"` y `end = "2026-10-06T18:45:00Z"`
**Then** B SHALL aparecer y A NO SHALL aparecer

#### Scenario: Orden por instante de inicio
**Given** un evento A con start_time="2026-10-06T19:00:00+02:00" (17:00Z)
**And** un evento B con start_time="2026-10-06T18:30:00Z"
**When** `list_by_date_range` se consulta con un rango que abarca los dos
**Then** A SHALL aparecer antes que B

#### Scenario: Una fila con timestamps ilegibles se incluye
**Given** una fila cuyo start_time no es parseable
**When** `list_by_date_range` se consulta con cualquier rango
**Then** la fila SHALL incluirse y SHALL quedar al final del orden

#### Scenario: Un límite no parseable no se convierte en instante
**When** `parse_range_bound("no-es-una-fecha", false)` se evalúa
**Then** SHALL devolver `None`

### Requirement: EventsRepo::find_free_slots SHALL comparar por instante

`EventsRepo::find_free_slots` SHALL calcular los huecos libres de un día comparando **instantes**
parseados, nunca el orden lexicográfico de las cadenas almacenadas, de modo que los eventos guardados
con offset local y los guardados con sufijo `Z` se ordenen y se recorten correctamente. El día SHALL
abarcar desde las `T00:00:00Z` del día indicado hasta el **inicio del día siguiente** (intervalo
semiabierto `[start, end)`, igual que el rango de consulta) y los huecos SHALL devolverse en RFC 3339
UTC canónico.

**Contracts:**

```rust
impl EventsRepo {
    /// `date` en `YYYY-MM-DD`. Los huecos se calculan por instante y se
    /// devuelven en RFC 3339 UTC canónico.
    pub async fn find_free_slots(
        pool: &SqlitePool,
        profile_id: &str,
        date: &str,
        duration_min: i64,
    ) -> Result<Vec<(String, String)>, sqlx::Error>;
}
```

**Scenarios:**

#### Scenario: Huecos con formatos mezclados
**Given** un evento con start_time="2026-10-06T19:00:00+02:00" y end_time="2026-10-06T20:00:00+02:00" (17:00–18:00Z)
**And** un evento con start_time="2026-10-06T18:30:00Z" y end_time="2026-10-06T19:30:00Z"
**When** se piden huecos de 30 minutos para `2026-10-06`
**Then** SHALL devolverse un hueco que termina en `2026-10-06T17:00:00Z`
**And** ningún hueco SHALL solaparse con los dos eventos
