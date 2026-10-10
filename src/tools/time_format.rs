/// Spanish day-of-week names (ISO weekday: 1 = Monday … 7 = Sunday).
pub const DIAS: [&str; 7] = [
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
    "domingo",
];

/// Spanish month names.
pub const MESES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

/// Spanish time-of-day phrases keyed by hour.
pub fn momento_del_dia(hora: u32) -> &'static str {
    match hora {
        0..=5 => "de la madrugada",
        6..=11 => "de la mañana",
        12..=20 => "de la tarde",
        _ => "de la noche",
    }
}

/// Parse an ISO‑8601 timestamp (with `Z`, an explicit offset, or as a naive
/// UTC value) into a UTC `DateTime`. Returns `None` when it does not parse.
fn parse_utc(iso: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::{DateTime, NaiveDateTime};

    let iso = iso.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(iso) {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    // Accept the `Z`-suffixed variants without an explicit offset.
    let naive =
        NaiveDateTime::parse_from_str(iso.trim_end_matches('Z'), "%Y-%m-%dT%H:%M:%S%.f").ok()?;
    Some(DateTime::from_naive_utc_and_offset(naive, chrono::Utc))
}

/// Convert a UTC instant and a timezone into an inline `"YYYY-MM-DD HH:MM"`
/// stamp in the effective timezone.
///
/// An invalid timezone falls back to UTC; an unparseable `iso_utc` yields
/// `None` so the caller can leave the message unstamped.
pub fn format_inline_timestamp(iso_utc: &str, tz: &str) -> Option<String> {
    use chrono_tz::Tz;
    use std::str::FromStr;

    let utc = parse_utc(iso_utc)?;
    let tz = Tz::from_str(tz).unwrap_or(chrono_tz::UTC);
    Some(utc.with_timezone(&tz).format("%Y-%m-%d %H:%M").to_string())
}

/// Convert a UTC instant and a timezone into the prompt clock string
/// `"YYYY-MM-DD HH:MM:SS (weekday)"`, with the Spanish weekday in lowercase.
///
/// An invalid timezone falls back to UTC; an unparseable `iso_utc` yields
/// `None` so the caller can fall back to a safe value.
pub fn format_prompt_now(iso_utc: &str, tz: &str) -> Option<String> {
    use chrono_tz::Tz;
    use std::str::FromStr;

    let utc = parse_utc(iso_utc)?;
    let tz = Tz::from_str(tz).unwrap_or(chrono_tz::UTC);
    let dt = utc.with_timezone(&tz);

    let wd = dt.format("%u").to_string().parse::<usize>().ok()?;
    let day_name = DIAS.get(wd - 1)?;
    Some(format!("{} ({})", dt.format("%Y-%m-%d %H:%M:%S"), day_name))
}

/// Format the current UTC time in the given timezone as a human‑readable
/// Spanish string, computed from the current system clock.
pub fn format_time_now(timezone: &str) -> String {
    use chrono::{Datelike, Timelike};
    use chrono_tz::Tz;
    use std::str::FromStr;

    let utc_now: chrono::DateTime<chrono::Utc> = chrono::Utc::now();

    // Fall back to UTC if the timezone string is invalid.
    let tz = Tz::from_str(timezone).unwrap_or(chrono_tz::UTC);
    let dt = utc_now.with_timezone(&tz);

    let wd = dt.format("%u").to_string().parse::<usize>().unwrap_or(1); // 1–7
    let day_name = DIAS[wd - 1];
    let month_name = MESES[dt.month0() as usize];
    let momento = momento_del_dia(dt.hour());

    format!(
        "Hoy es {}, {} de {} de {}, son las {}:{:02} {}",
        day_name,
        dt.day(),
        month_name,
        dt.year(),
        dt.hour(),
        dt.minute(),
        momento,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_momento_del_dia_madrugada() {
        assert_eq!(momento_del_dia(0), "de la madrugada");
        assert_eq!(momento_del_dia(5), "de la madrugada");
    }

    #[test]
    fn test_momento_del_dia_manana() {
        assert_eq!(momento_del_dia(6), "de la mañana");
        assert_eq!(momento_del_dia(11), "de la mañana");
    }

    #[test]
    fn test_momento_del_dia_tarde() {
        assert_eq!(momento_del_dia(12), "de la tarde");
        assert_eq!(momento_del_dia(20), "de la tarde");
    }

    #[test]
    fn test_momento_del_dia_noche() {
        assert_eq!(momento_del_dia(21), "de la noche");
        assert_eq!(momento_del_dia(23), "de la noche");
    }

    #[test]
    fn test_dias_length() {
        assert_eq!(DIAS.len(), 7);
    }

    #[test]
    fn test_meses_length() {
        assert_eq!(MESES.len(), 12);
    }

    #[test]
    fn test_format_time_now_returns_string() {
        let s = format_time_now("Europe/Madrid");
        assert!(s.starts_with("Hoy es "));
        assert!(s.contains("son las "));
    }

    #[test]
    fn test_format_time_now_invalid_tz_falls_back() {
        // Invalid timezone should fall back to UTC without panicking.
        let s = format_time_now("Bad/Zone");
        assert!(s.starts_with("Hoy es "));
    }

    // ─── temporal-awareness: inline timestamp + prompt clock ────────────────

    #[test]
    fn test_format_inline_timestamp_madrid() {
        // 20:15 UTC on 2026-07-15 = 22:15 CEST (Europe/Madrid, UTC+2).
        assert_eq!(
            format_inline_timestamp("2026-07-15T20:15:00Z", "Europe/Madrid"),
            Some("2026-07-15 22:15".to_string())
        );
    }

    #[test]
    fn test_format_inline_timestamp_invalid_tz_falls_back_to_utc() {
        assert_eq!(
            format_inline_timestamp("2026-07-15T20:15:00Z", "Bad/Zone"),
            Some("2026-07-15 20:15".to_string())
        );
    }

    #[test]
    fn test_format_inline_timestamp_unparseable_is_none() {
        assert_eq!(format_inline_timestamp("no-fecha", "Europe/Madrid"), None);
    }

    #[test]
    fn test_format_prompt_now_madrid() {
        // 17:00 UTC on 2026-10-09 = 19:00 CEST → Friday (viernes).
        assert_eq!(
            format_prompt_now("2026-10-09T17:00:00Z", "Europe/Madrid"),
            Some("2026-10-09 19:00:00 (viernes)".to_string())
        );
    }

    #[test]
    fn test_format_prompt_now_invalid_tz_falls_back_to_utc() {
        assert_eq!(
            format_prompt_now("2026-10-09T17:00:00Z", "Bad/Zone"),
            Some("2026-10-09 17:00:00 (viernes)".to_string())
        );
    }

    #[test]
    fn test_format_prompt_now_unparseable_is_none() {
        assert_eq!(format_prompt_now("nope", "Europe/Madrid"), None);
    }
}
