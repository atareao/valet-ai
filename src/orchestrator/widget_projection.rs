//! Deterministic widget projection over the assistant's final text.
//!
//! The server — not the model — decides presentation: it detects a presentation
//! block in the assistant's text, synthesizes the matching widget and records the
//! byte range (`span`) that widget occupies. Pure module: no I/O, no async.

use serde::Serialize;
use serde_json::{json, Map, Value};
use uuid::Uuid;

/// Semi-open byte range `[start, end)` over the assistant's persisted content.
/// Index 0 is `start`, index 1 is `end`. Serialises as the JSON array `[start, end]`.
pub type Span = [usize; 2];

/// A presentation block recognised in the assistant's text, with the byte range it occupies.
#[derive(Debug, Clone, PartialEq)]
pub enum Presentation {
    Options {
        title: String,
        options: Vec<String>,
        span: Span,
    },
    Checklist {
        title: String,
        items: Vec<String>,
        span: Span,
    },
    /// A place: a latitude/longitude pair.
    Location {
        title: String,
        latitude: f64,
        longitude: f64,
        span: Span,
    },
}

/// A widget ready to be persisted and pushed to the client.
/// `span` is `None` for widgets rendered by the model, and is omitted from the JSON.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Widget {
    pub id: String,
    pub name: String,
    pub data: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

/// A line of the input text together with the byte range it occupies.
///
/// Every offset is a **byte** offset into the original `text`; `end` points just past the line's
/// last character, so the trailing newline is always *outside* the range.
#[derive(Debug, Clone, Copy)]
struct Line<'a> {
    start: usize,
    text: &'a str,
    end: usize,
}

/// Splits `text` into lines, keeping every offset in bytes (never in characters).
/// A trailing `\r` (CRLF text) stays *outside* the line's range.
fn lines_of(text: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            let end = if index > start && text.as_bytes()[index - 1] == b'\r' {
                index - 1
            } else {
                index
            };
            lines.push(Line {
                start,
                text: &text[start..end],
                end,
            });
            start = index + 1;
        }
    }
    let end = if text.len() > start && text.as_bytes()[text.len() - 1] == b'\r' {
        text.len() - 1
    } else {
        text.len()
    };
    lines.push(Line {
        start,
        text: &text[start..end],
        end,
    });
    lines
}

/// Byte offset of the first space/tab-free character of `line`, relative to its start.
/// Used so a block's span begins at its first non-blank byte.
fn first_non_blank(line: &str) -> usize {
    line.bytes()
        .position(|byte| byte != b' ' && byte != b'\t')
        .unwrap_or(0)
}

/// Whether a line opens or closes a Markdown code fence (```` ``` ```` or `~~~`).
fn is_fence(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// Detect every presentation block in `text`, in order of appearance.
/// Empty vector when nothing is recognised. Spans never overlap.
pub fn detect(text: &str) -> Vec<Presentation> {
    let lines = lines_of(text);
    let mut presentations = Vec::new();
    let mut index = 0;
    let mut in_fence = false;

    while index < lines.len() {
        if is_fence(lines[index].text) {
            in_fence = !in_fence;
            index += 1;
            continue;
        }
        if in_fence {
            index += 1;
            continue;
        }

        let block = table_block(&lines, index)
            .or_else(|| checkbox_bullets_block(&lines, index))
            .or_else(|| letter_options_block(&lines, index))
            .or_else(|| question_options_block(&lines, index));

        match block {
            Some((presentation, next)) => {
                presentations.push(presentation);
                index = next;
            }
            None => {
                if let Some(presentation) = location_line(lines[index]) {
                    presentations.push(presentation);
                }
                index += 1;
            }
        }
    }

    presentations
}

/// Splits a Markdown table row into its cells, or `None` when the line is not a table row.
fn table_cells(line: &str) -> Option<Vec<&str>> {
    let trimmed = line.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }
    Some(
        trimmed[1..trimmed.len() - 1]
            .split('|')
            .map(str::trim)
            .collect(),
    )
}

/// Whether a table cell is an index header (`#`, `Nº`, `No.`…).
fn is_index_header(cell: &str) -> bool {
    let normalized = cell.trim().trim_end_matches('.').to_lowercase();
    matches!(
        normalized.as_str(),
        "#" | "nº" | "no" | "n°" | "num" | "núm" | "nro"
    )
}

/// Folds a single character to its unaccented lowercase ASCII-ish form.
fn fold_char(character: char) -> char {
    match character {
        'á' | 'à' | 'ä' | 'â' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'ñ' => 'n',
        other => other,
    }
}

/// Lowercases and strips accents so `Acción` and `accion` compare equal.
fn normalize_word(cell: &str) -> String {
    cell.trim()
        .trim_end_matches('.')
        .chars()
        .flat_map(|character| character.to_lowercase())
        .map(fold_char)
        .collect()
}

/// Whether a table cell names a task column (`Tarea`, `Acción`, `Task`, `Todo`…).
fn is_task_header(cell: &str) -> bool {
    matches!(
        normalize_word(cell).as_str(),
        "tarea"
            | "tareas"
            | "task"
            | "tasks"
            | "pendiente"
            | "pendientes"
            | "accion"
            | "acciones"
            | "todo"
            | "checklist"
    )
}

/// Whether a table cell is a checkbox marker.
fn is_checkbox_cell(cell: &str) -> bool {
    matches!(cell.trim(), "[ ]" | "[]" | "[x]" | "[X]" | "☐" | "☑")
}

/// Whether every cell is a Markdown table separator (`---`, `:--:`…).
fn is_separator_row(cells: &[&str]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let cell = cell.trim();
            !cell.is_empty() && cell.bytes().all(|byte| byte == b'-' || byte == b':')
        })
}

/// Recognises a Markdown checklist table starting at `index`: a run of table rows whose first
/// column is an index header or a checkbox.
fn table_block(lines: &[Line<'_>], index: usize) -> Option<(Presentation, usize)> {
    let mut end = index;
    while end < lines.len() && table_cells(lines[end].text).is_some() {
        end += 1;
    }
    if end == index {
        return None;
    }

    let rows = &lines[index..end];
    let first_cells = table_cells(rows[0].text)?;
    let index_header = matches!(first_cells.first(), Some(cell) if is_index_header(cell))
        && matches!(first_cells.get(1), Some(cell) if is_task_header(cell));

    let mut items = Vec::new();
    if index_header {
        for row in &rows[1..] {
            let cells = table_cells(row.text).unwrap_or_default();
            if cells.len() < 2 || is_separator_row(&cells) {
                continue;
            }
            if !cells[1].is_empty() {
                items.push(cells[1].to_string());
            }
        }
    } else {
        for row in rows {
            let cells = table_cells(row.text).unwrap_or_default();
            if cells.len() < 2 || !is_checkbox_cell(cells[0]) {
                continue;
            }
            if !cells[1].is_empty() {
                items.push(cells[1].to_string());
            }
        }
    }

    if items.len() < 2 {
        return None;
    }

    Some((
        Presentation::Checklist {
            title: String::new(),
            items,
            span: [
                rows[0].start + first_non_blank(rows[0].text),
                rows[rows.len() - 1].end,
            ],
        },
        end,
    ))
}

/// Text of a bullet line that carries a checkbox marker (`- [ ] …`, `- ☐ …`).
fn checkbox_bullet_text(line: &str) -> Option<&str> {
    let rest = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("• "))?
        .trim_start();

    for marker in ["[ ]", "[]", "[x]", "[X]", "☐", "☑"] {
        if let Some(text) = rest.strip_prefix(marker) {
            let text = text.trim();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

/// Recognises a run of at least two checkbox bullets as a checklist.
fn checkbox_bullets_block(lines: &[Line<'_>], index: usize) -> Option<(Presentation, usize)> {
    let mut end = index;
    while end < lines.len() && checkbox_bullet_text(lines[end].text).is_some() {
        end += 1;
    }
    if end - index < 2 {
        return None;
    }

    let items: Vec<String> = lines[index..end]
        .iter()
        .filter_map(|line| checkbox_bullet_text(line.text).map(str::to_string))
        .collect();

    Some((
        Presentation::Checklist {
            title: String::new(),
            items,
            span: [
                lines[index].start + first_non_blank(lines[index].text),
                lines[end - 1].end,
            ],
        },
        end,
    ))
}

/// The label letter and option text of a letter-labelled line (`A) …`, `Opción A: …`).
fn letter_label(line: &str) -> Option<(char, &str)> {
    if let Some(rest) = line
        .strip_prefix("Opción ")
        .or_else(|| line.strip_prefix("Opcion "))
    {
        let mut chars = rest.chars();
        if let Some(letter) = chars.next().filter(|c| c.is_ascii_alphabetic()) {
            let after = &rest[letter.len_utf8()..];
            let after = after
                .strip_prefix(':')
                .or_else(|| after.strip_prefix(')'))
                .or_else(|| after.strip_prefix('.'))
                .unwrap_or(after);
            let text = after.trim();
            if !text.is_empty() {
                return Some((letter.to_ascii_uppercase(), text));
            }
        }
        return None;
    }

    let bytes = line.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && matches!(bytes[1], b')' | b'.' | b':')
    {
        let text = line[2..].trim();
        if !text.is_empty() {
            return Some((char::from(bytes[0].to_ascii_uppercase()), text));
        }
    }
    None
}

/// Whether the label letters are a strictly ascending consecutive run (`A, B, C`…).
fn consecutive_ascending(labels: &[char]) -> bool {
    labels
        .windows(2)
        .all(|pair| pair[1] as u32 == pair[0] as u32 + 1)
}

/// Recognises a run of at least two consecutive letter-labelled lines as options
/// (no question needed). Gaps and repeats are rejected, which rules out roman
/// numerals (`I.`, `V.`) and repeated abbreviations (`U.S.`).
fn letter_options_block(lines: &[Line<'_>], index: usize) -> Option<(Presentation, usize)> {
    let mut end = index;
    let mut labels = Vec::new();
    while end < lines.len() {
        match letter_label(lines[end].text) {
            Some((letter, _)) => {
                labels.push(letter);
                end += 1;
            }
            None => break,
        }
    }
    if labels.len() < 2 || !consecutive_ascending(&labels) {
        return None;
    }

    let options: Vec<String> = lines[index..end]
        .iter()
        .filter_map(|line| letter_label(line.text).map(|(_, text)| text.to_string()))
        .collect();

    Some((
        Presentation::Options {
            title: String::new(),
            options,
            span: [
                lines[index].start + first_non_blank(lines[index].text),
                lines[end - 1].end,
            ],
        },
        end,
    ))
}

/// Text of a numbered (`1) …`, `1. …`) or bulleted (`- …`, `* …`, `• …`) list line.
fn list_item_text(line: &str) -> Option<&str> {
    for prefix in ["- ", "* ", "• "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            let text = rest.trim();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }

    let bytes = line.as_bytes();
    let mut digits = 0;
    while digits < bytes.len() && bytes[digits].is_ascii_digit() {
        digits += 1;
    }
    if digits > 0 && digits < bytes.len() && matches!(bytes[digits], b')' | b'.') {
        let text = line[digits + 1..].trim();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

/// Whether `needle` appears in `haystack` delimited by non-alphanumeric boundaries,
/// so `cual` matches `¿cuál…?` but not `cualquiera`.
fn contains_word(haystack: &str, needle: &str) -> bool {
    let mut search_from = 0;
    while let Some(offset) = haystack[search_from..].find(needle) {
        let start = search_from + offset;
        let end = start + needle.len();
        let before_ok = match haystack[..start].chars().next_back() {
            Some(character) => !character.is_alphanumeric(),
            None => true,
        };
        let after_ok = match haystack[end..].chars().next() {
            Some(character) => !character.is_alphanumeric(),
            None => true,
        };
        if before_ok && after_ok {
            return true;
        }
        search_from = end;
    }
    false
}

/// Whether a line closes a list of alternatives with a «¿cuál…?»-style question.
/// Only that family counts: a plain yes/no question leaves a procedure as a procedure.
fn is_interrogative(line: &str) -> bool {
    let trimmed = line.trim();
    if !(trimmed.contains('¿') || trimmed.ends_with('?')) {
        return false;
    }
    let lower = trimmed.to_lowercase();
    contains_word(&lower, "cuál") || contains_word(&lower, "cual")
}

/// Recognises a run of numbered/bulleted lines closed by an interrogative line as options.
fn question_options_block(lines: &[Line<'_>], index: usize) -> Option<(Presentation, usize)> {
    let mut end = index;
    while end < lines.len() && list_item_text(lines[end].text).is_some() {
        end += 1;
    }
    if end - index < 2 {
        return None;
    }
    match lines.get(end) {
        Some(line) if is_interrogative(line.text) => {}
        _ => return None,
    }

    let options: Vec<String> = lines[index..end]
        .iter()
        .filter_map(|line| list_item_text(line.text).map(str::to_string))
        .collect();

    Some((
        Presentation::Options {
            title: String::new(),
            options,
            span: [
                lines[index].start + first_non_blank(lines[index].text),
                lines[end - 1].end,
            ],
        },
        end,
    ))
}

/// Number of digits after the decimal point, or `None` when the token is not a decimal number.
fn decimal_digits(token: &str) -> Option<usize> {
    let (_, fraction) = token.split_once('.')?;
    if !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        Some(fraction.len())
    } else {
        None
    }
}

/// A bare `num, num` line whose two decimals carry at least four digits each.
fn decimal_pair(line: &str) -> Option<(f64, f64)> {
    let trimmed = line.trim();
    let mut parts = trimmed.split(',');
    let latitude = parts.next()?.trim();
    let longitude = parts.next()?.trim();
    if parts.next().is_some() {
        return None;
    }
    if decimal_digits(latitude)? < 4 || decimal_digits(longitude)? < 4 {
        return None;
    }
    Some((latitude.parse().ok()?, longitude.parse().ok()?))
}

/// Parses a leading number (`-3.7038`, `40.4168`, `+1.5`) off `text`.
fn parse_number(text: &str) -> Option<f64> {
    let end = text
        .find(|character: char| {
            !(character.is_ascii_digit()
                || character == '.'
                || character == '-'
                || character == '+')
        })
        .unwrap_or(text.len());
    if end == 0 {
        return None;
    }
    text[..end].parse().ok()
}

/// The value of `label` on `line` together with its byte position: the label must be a whole
/// word, followed by optional spaces and a `:` or `=`, and then a number.
fn label_value(line: &str, label: &str) -> Option<(usize, f64)> {
    let mut search_from = 0;
    while let Some(offset) = line[search_from..].find(label) {
        let start = search_from + offset;
        let end = start + label.len();
        let before_ok = match line[..start].chars().next_back() {
            Some(character) => !character.is_alphanumeric(),
            None => true,
        };
        if before_ok {
            let rest = line[end..].trim_start();
            let after = rest.strip_prefix(':').or_else(|| rest.strip_prefix('='));
            if let Some(after) = after {
                if let Some(value) = parse_number(after.trim_start()) {
                    return Some((start, value));
                }
            }
        }
        search_from = end;
    }
    None
}

/// The first number labelled with any of `labels` (whole word) on the line.
fn labelled_value(line: &str, labels: &[&str]) -> Option<f64> {
    labels
        .iter()
        .filter_map(|label| label_value(line, label))
        .min_by_key(|(position, _)| *position)
        .map(|(_, value)| value)
}

/// A labelled latitude/longitude pair (`lat: 40.4168, lon: -3.7038`). Both are required.
fn labelled_pair(line: &str) -> Option<(f64, f64)> {
    let latitude = labelled_value(line, &["lat", "latitude", "latitud"])?;
    let longitude = labelled_value(line, &["lon", "lng", "long", "longitude", "longitud"])?;
    Some((latitude, longitude))
}

/// Recognises a single line as a location: a labelled pair or a bare decimal pair.
fn location_line(line: Line<'_>) -> Option<Presentation> {
    let (latitude, longitude) = labelled_pair(line.text).or_else(|| decimal_pair(line.text))?;
    Some(Presentation::Location {
        title: String::new(),
        latitude,
        longitude,
        span: [line.start + first_non_blank(line.text), line.end],
    })
}

/// Tool names whose `data` may supply the items of a projected widget.
const ITEM_TOOLS: &[&str] = &[
    "tasks",
    "notifications",
    "email_list_unread",
    "rss_list_unread",
];

/// Keys, in order, that may hold the list of items inside a tool's `data`.
const LIST_KEYS: &[&str] = &[
    "tasks",
    "notifications",
    "messages",
    "entries",
    "items",
    "results",
];

/// Keys, in order, that may hold the label of a single item.
const LABEL_KEYS: &[&str] = &[
    "title", "subject", "label", "name", "summary", "message", "text",
];

/// Default label of the `select` field when the presentation carries no title.
const DEFAULT_OPTION_LABEL: &str = "Elige una opción";

/// Build widgets from the detected presentations. `tool_results` carries the
/// `(tool_name, data)` of the tools the turn called.
pub fn synthesize(presentations: &[Presentation], tool_results: &[(String, Value)]) -> Vec<Widget> {
    presentations
        .iter()
        .map(|presentation| widget_for(presentation, tool_results))
        .collect()
}

/// Builds the widget for a single presentation, preferring the turn's tool data when it fits.
fn widget_for(presentation: &Presentation, tool_results: &[(String, Value)]) -> Widget {
    match presentation {
        Presentation::Options {
            title,
            options,
            span,
        } => {
            let label = if title.is_empty() {
                DEFAULT_OPTION_LABEL
            } else {
                title.as_str()
            };
            let mut data = Map::new();
            if !title.is_empty() {
                data.insert("title".to_string(), Value::String(title.clone()));
            }
            data.insert(
                "fields".to_string(),
                json!([{
                    "name": "choice",
                    "label": label,
                    "type": "select",
                    "options": options,
                }]),
            );
            data.insert(
                "submit_label".to_string(),
                Value::String("Enviar".to_string()),
            );
            widget("QuickForm", Value::Object(data), *span)
        }
        Presentation::Checklist { title, items, span } => {
            let labels = tool_items(tool_results).unwrap_or_else(|| items.clone());
            let entries: Vec<Value> = labels
                .iter()
                .enumerate()
                .map(|(position, label)| {
                    json!({ "id": format!("item-{}", position + 1), "label": label })
                })
                .collect();
            let mut data = Map::new();
            if !title.is_empty() {
                data.insert("title".to_string(), Value::String(title.clone()));
            }
            data.insert("items".to_string(), Value::Array(entries));
            widget("Checklist", Value::Object(data), *span)
        }
        Presentation::Location {
            title,
            latitude,
            longitude,
            span,
        } => {
            let mut data = Map::new();
            if !title.is_empty() {
                data.insert("title".to_string(), Value::String(title.clone()));
            }
            data.insert("latitude".to_string(), json!(*latitude));
            data.insert("longitude".to_string(), json!(*longitude));
            widget("LocationWidget", Value::Object(data), *span)
        }
    }
}

/// Assembles a projected widget with a fresh identity and its span.
fn widget(name: &str, data: Value, span: Span) -> Widget {
    Widget {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        data,
        span: Some(span),
    }
}

/// Labels of the first admitted tool list that carries at least two non-empty labels.
fn tool_items(tool_results: &[(String, Value)]) -> Option<Vec<String>> {
    for (name, data) in tool_results {
        if !ITEM_TOOLS.contains(&name.as_str()) {
            continue;
        }
        let labels: Vec<String> = list_values(data).iter().filter_map(label_of).collect();
        if labels.len() >= 2 {
            return Some(labels);
        }
    }
    None
}

/// The first non-empty list inside a tool's `data` (or `data` itself when it is an array).
fn list_values(data: &Value) -> Vec<Value> {
    if let Some(array) = data.as_array() {
        return array.clone();
    }
    for key in LIST_KEYS {
        if let Some(array) = data.get(*key).and_then(Value::as_array) {
            if !array.is_empty() {
                return array.clone();
            }
        }
    }
    Vec::new()
}

/// The label of a single item: the first present field, or the string itself.
fn label_of(item: &Value) -> Option<String> {
    if let Some(text) = item.as_str() {
        let text = text.trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
        return None;
    }
    for key in LABEL_KEYS {
        if let Some(text) = item.get(*key).and_then(Value::as_str) {
            let text = text.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns the semi-open byte range of any presentation.
    fn span_of(presentation: &Presentation) -> Span {
        match presentation {
            Presentation::Options { span, .. }
            | Presentation::Checklist { span, .. }
            | Presentation::Location { span, .. } => *span,
        }
    }

    // ---------------------------------------------------------------------
    // Detection — Options
    // ---------------------------------------------------------------------

    #[test]
    fn options_letter_paren() {
        let text = "A) Rojo\nB) Azul";
        assert_eq!(
            detect(text),
            vec![Presentation::Options {
                title: String::new(),
                options: vec!["Rojo".to_string(), "Azul".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn options_letter_dot() {
        let text = "A. Uno\nB. Dos";
        assert_eq!(
            detect(text),
            vec![Presentation::Options {
                title: String::new(),
                options: vec!["Uno".to_string(), "Dos".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn options_letter_colon() {
        let text = "A: Primero\nB: Segundo";
        assert_eq!(
            detect(text),
            vec![Presentation::Options {
                title: String::new(),
                options: vec!["Primero".to_string(), "Segundo".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn options_opcion_a_label() {
        let text = "Opción A: Primero\nOpción B: Segundo";
        assert_eq!(
            detect(text),
            vec![Presentation::Options {
                title: String::new(),
                options: vec!["Primero".to_string(), "Segundo".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn options_numbered_paren_with_question() {
        let block = "1) Rojo\n2) Azul";
        let text = format!("{block}\n¿Cuál prefieres?");
        assert_eq!(
            detect(&text),
            vec![Presentation::Options {
                title: String::new(),
                options: vec!["Rojo".to_string(), "Azul".to_string()],
                span: [0, block.len()],
            }]
        );
    }

    #[test]
    fn options_numbered_dot_with_question() {
        let block = "1. Rojo\n2. Azul";
        let text = format!("{block}\n¿Cuál eliges?");
        assert_eq!(
            detect(&text),
            vec![Presentation::Options {
                title: String::new(),
                options: vec!["Rojo".to_string(), "Azul".to_string()],
                span: [0, block.len()],
            }]
        );
    }

    #[test]
    fn options_bullets_with_question() {
        let block = "- Rojo\n- Azul";
        let text = format!("{block}\n¿Cuál prefieres?");
        assert_eq!(
            detect(&text),
            vec![Presentation::Options {
                title: String::new(),
                options: vec!["Rojo".to_string(), "Azul".to_string()],
                span: [0, block.len()],
            }]
        );
    }

    // ---------------------------------------------------------------------
    // Detection — Checklist
    // ---------------------------------------------------------------------

    #[test]
    fn checklist_markdown_table_with_index() {
        let text = "| # | Tarea |\n|---|---|\n| 1 | Comprar pan |\n| 2 | Pagar luz |";
        assert_eq!(
            detect(text),
            vec![Presentation::Checklist {
                title: String::new(),
                items: vec!["Comprar pan".to_string(), "Pagar luz".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn checklist_markdown_table_with_numero_index() {
        let text = "| Nº | Tarea |\n|---|---|\n| 1 | Uno |\n| 2 | Dos |";
        assert_eq!(
            detect(text),
            vec![Presentation::Checklist {
                title: String::new(),
                items: vec!["Uno".to_string(), "Dos".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn checklist_markdown_table_with_checkbox_column() {
        let text = "| [ ] | Llamar a Ana |\n| [x] | Pagar la luz |";
        assert_eq!(
            detect(text),
            vec![Presentation::Checklist {
                title: String::new(),
                items: vec!["Llamar a Ana".to_string(), "Pagar la luz".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn checklist_bullets_with_checkbox() {
        let text = "- [ ] Comprar pan\n- [x] Sacar la basura";
        assert_eq!(
            detect(text),
            vec![Presentation::Checklist {
                title: String::new(),
                items: vec!["Comprar pan".to_string(), "Sacar la basura".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn checklist_bullets_with_box_symbol() {
        let text = "- ☐ Uno\n- ☐ Dos";
        assert_eq!(
            detect(text),
            vec![Presentation::Checklist {
                title: String::new(),
                items: vec!["Uno".to_string(), "Dos".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    // ---------------------------------------------------------------------
    // Detection — Location
    // ---------------------------------------------------------------------

    #[test]
    fn location_labelled_lat_lon_pair() {
        let text = "lat: 40.4168, lon: -3.7038";
        assert_eq!(
            detect(text),
            vec![Presentation::Location {
                title: String::new(),
                latitude: 40.4168,
                longitude: -3.7038,
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn location_decimal_pair() {
        let text = "40.4168, -3.7038";
        assert_eq!(
            detect(text),
            vec![Presentation::Location {
                title: String::new(),
                latitude: 40.4168,
                longitude: -3.7038,
                span: [0, text.len()],
            }]
        );
    }

    // ---------------------------------------------------------------------
    // Contract of `span`
    // ---------------------------------------------------------------------

    #[test]
    fn span_delimits_block_with_heading_and_question_outside() {
        let heading = "Elige una opción:\n";
        let block = "A) Café\nB) Té";
        let question = "\n¿Cuál eliges?";
        let text = format!("{heading}{block}{question}");

        let detected = detect(&text);
        assert_eq!(detected.len(), 1);

        let [start, end] = span_of(&detected[0]);
        assert_eq!(
            &text[start..end],
            block,
            "el tramo SHALL ser exactamente el bloque"
        );
        assert!(
            !text[..start].contains("A)"),
            "el encabezado previo queda fuera del tramo"
        );
        assert!(
            !text[end..].contains("B)"),
            "la pregunta posterior queda fuera del tramo"
        );

        // Starts at the first byte of the block (the `A`) and ends right after the last
        // character of its last line — the trailing `\n` stays outside.
        assert_eq!(start, heading.len());
        assert_eq!(end, heading.len() + block.len());
        assert_eq!(text.as_bytes()[start], b'A');
        assert_eq!(text.as_bytes()[end], b'\n');

        match &detected[0] {
            Presentation::Options { options, .. } => {
                assert_eq!(options, &["Café".to_string(), "Té".to_string()]);
            }
            other => panic!("se esperaba Options, se obtuvo {other:?}"),
        }
    }

    #[test]
    fn two_blocks_produce_non_overlapping_spans_in_order() {
        let table = "| # | Tarea |\n|---|---|\n| 1 | Uno |\n| 2 | Dos |";
        let options = "A) Rojo\nB) Azul";
        let text = format!("{table}\n{options}");

        let detected = detect(&text);
        assert_eq!(
            detected.len(),
            2,
            "se esperaban dos bloques: una tabla y unas opciones"
        );

        let [s0, e0] = span_of(&detected[0]);
        let [s1, e1] = span_of(&detected[1]);
        assert_eq!(&text[s0..e0], table, "el primer tramo es la tabla entera");
        assert_eq!(&text[s1..e1], options, "el segundo tramo son las opciones");
        assert!(e0 <= s1, "los tramos no se solapan: {e0} <= {s1}");
    }

    #[test]
    fn span_offsets_are_utf8_bytes_not_characters() {
        // Two multibyte `é` sit just before the block, so the byte offset of the block
        // start is strictly greater than its character offset.
        let text = "éé\nA) Uno\nB) Dos";

        let detected = detect(text);
        assert_eq!(detected.len(), 1);

        let [start, end] = span_of(&detected[0]);
        assert_eq!(
            &text[start..end],
            "A) Uno\nB) Dos",
            "content[start..end] corta sin pánico y devuelve el bloque"
        );

        let char_start = text[..start].chars().count();
        assert_eq!(char_start, 3);
        assert!(
            start > char_start,
            "el offset del span es de bytes ({start}), no de caracteres ({char_start})"
        );
        assert!(
            !text.is_char_boundary(char_start),
            "un corte por caracteres caería dentro de 'é' y entraría en pánico"
        );

        match &detected[0] {
            Presentation::Options { options, .. } => {
                assert_eq!(options, &["Uno".to_string(), "Dos".to_string()]);
            }
            other => panic!("se esperaba Options, se obtuvo {other:?}"),
        }
    }

    #[test]
    fn span_excludes_trailing_cr_with_crlf() {
        let text = "A) Uno\r\nB) Dos\r\n";

        let detected = detect(text);
        assert_eq!(detected.len(), 1);

        let [start, end] = span_of(&detected[0]);
        assert_eq!(
            [start, end],
            [0, 14],
            "el tramo no incluye el `\\r` de la última línea"
        );
        assert_eq!(&text[start..end], "A) Uno\r\nB) Dos");
        assert!(
            !text[start..end].ends_with('\r'),
            "el tramo no SHALL terminar en `\\r`"
        );
    }

    #[test]
    fn span_starts_at_first_non_blank_byte_of_block() {
        // The block is indented; the span starts at the first non-blank byte of its first line.
        let text = "  | # | Tarea |\n  |---|---|\n  | 1 | Uno |\n  | 2 | Dos |";

        let detected = detect(text);
        assert_eq!(detected.len(), 1);

        let [start, end] = span_of(&detected[0]);
        assert_eq!(start, 2, "el tramo empieza tras la indentación");
        assert_eq!(
            &text[start..end],
            "| # | Tarea |\n  |---|---|\n  | 1 | Uno |\n  | 2 | Dos |"
        );
    }

    // ---------------------------------------------------------------------
    // NO detection — the real risk
    // ---------------------------------------------------------------------

    #[test]
    fn no_detection_single_time() {
        assert!(detect("Son las 18:30").is_empty());
    }

    #[test]
    fn no_detection_plain_prose() {
        assert!(detect("La capital de Francia es París y el Sena la cruza.").is_empty());
    }

    #[test]
    fn no_detection_table_without_index_or_checkbox() {
        assert!(detect("| Ciudad | Habitantes |\n|---|---|\n| Madrid | 3.2 |").is_empty());
    }

    #[test]
    fn no_detection_single_decimal_number() {
        assert!(detect("el resultado es 3.14159").is_empty());
    }

    #[test]
    fn no_detection_enumeration_within_sentence() {
        assert!(detect("puedes elegir entre A y B, pero yo prefiero A").is_empty());
    }

    #[test]
    fn no_detection_numbered_procedure() {
        assert!(detect("1. Abre el fichero\n2. Edita la línea\n3. Guarda").is_empty());
    }

    #[test]
    fn no_detection_empty_string() {
        assert!(detect("").is_empty());
    }

    #[test]
    fn no_detection_whitespace_only() {
        assert!(detect("   \n  ").is_empty());
    }

    #[test]
    fn no_detection_address_without_coordinates() {
        // D1: a bare address has no coordinates, so it is not a `Location` in the v1.
        assert!(detect("Calle Mayor 5, Madrid").is_empty());
    }

    #[test]
    fn no_detection_fenced_code_block_with_letter_options() {
        assert!(detect("```\nA) uno\nB) dos\n```").is_empty());
    }

    #[test]
    fn no_detection_fenced_code_block_with_table() {
        assert!(detect("```\n| # | col |\n|---|---|\n| 1 | a |\n| 2 | b |\n```").is_empty());
    }

    #[test]
    fn no_detection_procedure_with_yes_no_question() {
        assert!(detect("1. Abre el fichero\n2. Edita la línea\n¿Quieres que continúe?").is_empty());
    }

    #[test]
    fn no_detection_prose_mentioning_lat_lon_with_numbers() {
        assert!(detect("La relación entre latitud y longitud es 2.5 y 3.7").is_empty());
    }

    #[test]
    fn no_detection_roman_numeral_outline() {
        assert!(detect("I. Introducción\nV. Conclusión").is_empty());
    }

    #[test]
    fn no_detection_repeated_letter_abbreviation_lines() {
        assert!(detect("U.S. markets fell\nU.S. dollar rose").is_empty());
    }

    #[test]
    fn no_detection_indexed_table_of_non_tasks() {
        let text =
            "| # | Película | Año |\n|---|---|---|\n| 1 | Matrix | 1999 |\n| 2 | Alien | 1979 |";
        assert!(detect(text).is_empty());
    }

    // ---------------------------------------------------------------------
    // Detection — fenced code blocks are skipped
    // ---------------------------------------------------------------------

    #[test]
    fn detection_resumes_after_fenced_code_block() {
        let text = "```\nA) uno\nB) dos\n```\nA) Rojo\nB) Azul";

        let detected = detect(text);
        assert_eq!(
            detected.len(),
            1,
            "solo se detecta el bloque fuera del fence"
        );
        match &detected[0] {
            Presentation::Options { options, .. } => {
                assert_eq!(options, &["Rojo".to_string(), "Azul".to_string()]);
            }
            other => panic!("se esperaba Options, se obtuvo {other:?}"),
        }
    }

    #[test]
    fn detection_skips_tilde_fenced_code_block() {
        assert!(detect("~~~\nA) uno\nB) dos\n~~~").is_empty());
    }

    // ---------------------------------------------------------------------
    // Detection — branch coverage
    // ---------------------------------------------------------------------

    #[test]
    fn checklist_bullets_with_star_and_box_symbol() {
        let text = "* [ ] Uno\n• [x] Dos";
        assert_eq!(
            detect(text),
            vec![Presentation::Checklist {
                title: String::new(),
                items: vec!["Uno".to_string(), "Dos".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn checklist_bullets_with_empty_box() {
        let text = "- [] Uno\n- [X] Dos";
        assert_eq!(
            detect(text),
            vec![Presentation::Checklist {
                title: String::new(),
                items: vec!["Uno".to_string(), "Dos".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn checklist_bullets_with_checked_box_symbol() {
        let text = "- ☑ Uno\n- ☑ Dos";
        assert_eq!(
            detect(text),
            vec![Presentation::Checklist {
                title: String::new(),
                items: vec!["Uno".to_string(), "Dos".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn options_bullets_with_star_and_bullet_symbol() {
        let block = "* Rojo\n• Azul";
        let text = format!("{block}\n¿Cuál prefieres?");
        assert_eq!(
            detect(&text),
            vec![Presentation::Options {
                title: String::new(),
                options: vec!["Rojo".to_string(), "Azul".to_string()],
                span: [0, block.len()],
            }]
        );
    }

    #[test]
    fn checklist_table_with_num_index() {
        let text = "| num | Tarea |\n|---|---|\n| 1 | Uno |\n| 2 | Dos |";
        assert_eq!(
            detect(text),
            vec![Presentation::Checklist {
                title: String::new(),
                items: vec!["Uno".to_string(), "Dos".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn checklist_table_with_nro_index() {
        let text = "| Nro | Tarea |\n|---|---|\n| 1 | Uno |\n| 2 | Dos |";
        assert_eq!(
            detect(text),
            vec![Presentation::Checklist {
                title: String::new(),
                items: vec!["Uno".to_string(), "Dos".to_string()],
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn location_labelled_lng_pair() {
        let text = "lat: 40.4168, lng: -3.7038";
        assert_eq!(
            detect(text),
            vec![Presentation::Location {
                title: String::new(),
                latitude: 40.4168,
                longitude: -3.7038,
                span: [0, text.len()],
            }]
        );
    }

    #[test]
    fn location_labelled_longitude_pair() {
        let text = "latitude: 40.4168, longitude: -3.7038";
        assert_eq!(
            detect(text),
            vec![Presentation::Location {
                title: String::new(),
                latitude: 40.4168,
                longitude: -3.7038,
                span: [0, text.len()],
            }]
        );
    }

    // ---------------------------------------------------------------------
    // synthesize
    // ---------------------------------------------------------------------

    #[test]
    fn synthesize_options_without_tool_uses_text_options() {
        let text = "A) Rojo\nB) Azul";
        let widgets = synthesize(&detect(text), &[]);

        assert_eq!(widgets.len(), 1);
        assert_eq!(widgets[0].name, "QuickForm");
        assert_eq!(
            widgets[0].data,
            json!({
                "fields": [{
                    "name": "choice",
                    "label": "Elige una opción",
                    "type": "select",
                    "options": ["Rojo", "Azul"],
                }],
                "submit_label": "Enviar",
            })
        );
        assert_eq!(widgets[0].span, Some([0, text.len()]));
    }

    #[test]
    fn synthesize_options_with_title_keeps_it_and_labels_the_field() {
        let presentations = vec![Presentation::Options {
            title: "¿Qué bebes?".to_string(),
            options: vec!["Café".to_string(), "Té".to_string()],
            span: [0, 20],
        }];
        let widgets = synthesize(&presentations, &[]);

        assert_eq!(
            widgets[0].data,
            json!({
                "title": "¿Qué bebes?",
                "fields": [{
                    "name": "choice",
                    "label": "¿Qué bebes?",
                    "type": "select",
                    "options": ["Café", "Té"],
                }],
                "submit_label": "Enviar",
            })
        );
    }

    #[test]
    fn synthesize_checklist_with_tool_data_uses_tool_labels() {
        let text = "| # | Tarea |\n|---|---|\n| 1 | Comprar pan |\n| 2 | Pagar luz |";
        let tool_results = vec![(
            "notifications".to_string(),
            json!({ "notifications": [{ "title": "Aviso A" }, { "title": "Aviso B" }], "total": 2 }),
        )];
        let widgets = synthesize(&detect(text), &tool_results);

        assert_eq!(widgets.len(), 1);
        assert_eq!(widgets[0].name, "Checklist");
        assert_eq!(
            widgets[0].data,
            json!({
                "items": [
                    { "id": "item-1", "label": "Aviso A" },
                    { "id": "item-2", "label": "Aviso B" },
                ],
            })
        );
        assert_eq!(widgets[0].span, Some([0, text.len()]));
    }

    #[test]
    fn synthesize_checklist_without_tool_uses_text_items() {
        let text = "- [ ] Comprar pan\n- [x] Sacar la basura";
        let widgets = synthesize(&detect(text), &[]);

        assert_eq!(widgets.len(), 1);
        assert_eq!(widgets[0].name, "Checklist");
        assert_eq!(
            widgets[0].data,
            json!({
                "items": [
                    { "id": "item-1", "label": "Comprar pan" },
                    { "id": "item-2", "label": "Sacar la basura" },
                ],
            })
        );
        assert_eq!(widgets[0].span, Some([0, text.len()]));
    }

    #[test]
    fn synthesize_options_ignore_short_tool_list() {
        let text = "A) Rojo\nB) Azul";
        let tool_results = vec![(
            "tasks".to_string(),
            json!({ "count": 1, "tasks": [{ "title": "Solo una" }], "summary": "" }),
        )];
        let widgets = synthesize(&detect(text), &tool_results);

        assert_eq!(
            widgets[0].data["fields"][0]["options"],
            json!(["Rojo", "Azul"])
        );
    }

    #[test]
    fn synthesize_options_always_use_text_options() {
        // F6: tool data only feeds `Checklist`; a task tool does not produce the option list.
        let text = "A) Rojo\nB) Azul";
        let tool_results = vec![(
            "tasks".to_string(),
            json!({
                "count": 2,
                "tasks": [{ "title": "Comprar pan" }, { "title": "Pagar luz" }],
                "summary": "2 tareas",
            }),
        )];
        let widgets = synthesize(&detect(text), &tool_results);

        assert_eq!(
            widgets[0].data["fields"][0]["options"],
            json!(["Rojo", "Azul"])
        );
    }

    #[test]
    fn synthesize_ignores_unadmitted_tools() {
        let text = "A) Rojo\nB) Azul";
        let tool_results = vec![(
            "weather".to_string(),
            json!({ "items": [{ "title": "Sol" }, { "title": "Lluvia" }] }),
        )];
        let widgets = synthesize(&detect(text), &tool_results);

        assert_eq!(
            widgets[0].data["fields"][0]["options"],
            json!(["Rojo", "Azul"])
        );
    }

    #[test]
    fn synthesize_location_with_coordinates() {
        let text = "40.4168, -3.7038";
        let widgets = synthesize(&detect(text), &[]);

        assert_eq!(widgets.len(), 1);
        assert_eq!(widgets[0].name, "LocationWidget");
        assert_eq!(
            widgets[0].data,
            json!({ "latitude": 40.4168, "longitude": -3.7038 })
        );
        assert_eq!(widgets[0].span, Some([0, text.len()]));
    }

    #[test]
    fn synthesize_preserves_each_span_and_gives_unique_ids() {
        let text = "| # | Tarea |\n|---|---|\n| 1 | Uno |\n| 2 | Dos |\nA) Rojo\nB) Azul";
        let presentations = detect(text);
        let widgets = synthesize(&presentations, &[]);

        assert_eq!(widgets.len(), 2);
        assert_eq!(widgets[0].span, Some(span_of(&presentations[0])));
        assert_eq!(widgets[1].span, Some(span_of(&presentations[1])));
        assert_ne!(widgets[0].id, widgets[1].id);
    }

    #[test]
    fn synthesize_checklist_with_title_keeps_it() {
        let presentations = vec![Presentation::Checklist {
            title: "Tareas".to_string(),
            items: vec!["Uno".to_string(), "Dos".to_string()],
            span: [0, 10],
        }];
        let widgets = synthesize(&presentations, &[]);

        assert_eq!(
            widgets[0].data,
            json!({
                "title": "Tareas",
                "items": [
                    { "id": "item-1", "label": "Uno" },
                    { "id": "item-2", "label": "Dos" },
                ],
            })
        );
    }

    #[test]
    fn synthesize_location_with_title_keeps_it() {
        let presentations = vec![Presentation::Location {
            title: "Casa".to_string(),
            latitude: 40.4168,
            longitude: -3.7038,
            span: [0, 18],
        }];
        let widgets = synthesize(&presentations, &[]);

        assert_eq!(
            widgets[0].data,
            json!({ "title": "Casa", "latitude": 40.4168, "longitude": -3.7038 })
        );
    }

    #[test]
    fn synthesize_checklist_skips_short_tool_list_and_takes_next() {
        let text = "| # | Tarea |\n|---|---|\n| 1 | Uno |\n| 2 | Dos |";
        let tool_results = vec![
            (
                "tasks".to_string(),
                json!({ "count": 1, "tasks": [{ "title": "Solo una" }], "summary": "" }),
            ),
            (
                "notifications".to_string(),
                json!({ "notifications": [{ "title": "Aviso A" }, { "title": "Aviso B" }], "total": 2 }),
            ),
        ];
        let widgets = synthesize(&detect(text), &tool_results);

        assert_eq!(widgets[0].data["items"][0]["label"], json!("Aviso A"));
        assert_eq!(widgets[0].data["items"][1]["label"], json!("Aviso B"));
    }
}
