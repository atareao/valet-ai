//! Estimador determinista de tokens para cadenas JSON.
//!
//! Sin dependencias, sin red y sin I/O: recorre el JSON como JSON y cuenta
//! símbolos estructurales, cadenas (con ratio distinta para ASCII y no-ASCII),
//! literales `true`/`false`/`null` y números. Es una aproximación consciente
//! —Valet habla con varios proveedores y ningún tokenizer sería exacto para
//! todos— pero acotada y reproducible.

/// Estimate the number of tokens of a JSON string.
///
/// The walk is structural: whitespace is collapsed, string contents are
/// estimated by their characters (a different ratio for ASCII and non-ASCII)
/// and escaped quotes (`\"`) are treated as content, never as the end of the
/// string. The same input always yields the same result.
pub fn estimate_json_tokens(json: &str) -> usize {
    let mut token_count = 0;
    let mut in_string = false;
    let mut current_string = String::new();
    let mut is_escaped = false;

    let chars: Vec<char> = json.chars().collect();
    let mut idx = 0;

    while idx < chars.len() {
        let c = chars[idx];

        if in_string {
            if is_escaped {
                // Aproximación conocida (m2): los escapes NO se decodifican.
                // `\uXXXX` deja pasar los caracteres literales (`u`, `0`, `0`,
                // `e`, `9`), de modo que `\u00e9` cuesta 5 caracteres en lugar
                // de 1. Es deliberado: el JSON que mide la Capa C procede de
                // `serde_json::to_string`, que emite el no-ASCII en crudo y
                // nunca lo escapa como `\uXXXX`. No se corrige para no añadir
                // un decodificador que la entrada real no necesita.
                current_string.push(c);
                is_escaped = false;
            } else if c == '\\' {
                is_escaped = true;
            } else if c == '"' {
                token_count += estimate_string_tokens(&current_string);
                token_count += 2; // opening and closing quotes
                current_string.clear();
                in_string = false;
            } else {
                current_string.push(c);
            }
            idx += 1;
            continue;
        }

        match c {
            '"' => {
                in_string = true;
            }
            '{' | '}' | '[' | ']' | ':' | ',' => {
                token_count += 1;
            }
            ' ' | '\t' | '\r' | '\n' => {
                let start = idx;
                while idx < chars.len() && matches!(chars[idx], ' ' | '\t' | '\r' | '\n') {
                    idx += 1;
                }
                let whitespace_len = idx - start;
                token_count += whitespace_len.div_ceil(4);
                continue;
            }
            _ => {
                let start = idx;
                while idx < chars.len()
                    && !matches!(
                        chars[idx],
                        '{' | '}' | '[' | ']' | ':' | ',' | '"' | ' ' | '\t' | '\r' | '\n'
                    )
                {
                    idx += 1;
                }
                let token_str: String = chars[start..idx].iter().collect();
                token_count += estimate_literal_tokens(&token_str);
                continue;
            }
        }
        idx += 1;
    }
    token_count
}

/// Estimate the tokens of a JSON string's contents (quotes excluded).
fn estimate_string_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let char_count = text.chars().count();
    let non_ascii_count = text.chars().filter(|c| !c.is_ascii()).count();
    if non_ascii_count > 0 {
        ((char_count as f64) / 3.2).ceil() as usize
    } else {
        ((char_count as f64) / 4.0).ceil() as usize
    }
}

/// Estimate the tokens of a JSON literal or number token.
fn estimate_literal_tokens(literal: &str) -> usize {
    match literal {
        "true" | "false" | "null" => 1,
        _ => {
            let digits = literal.chars().filter(|c| c.is_ascii_digit()).count();
            if digits == 0 {
                1
            } else {
                ((digits as f64) / 2.5).ceil() as usize
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Aggregate token stats read from `llm_requests` for the calibration report.
    type LlmRequestAggregates = (i64, Option<f64>, Option<f64>, Option<f64>);

    // ─── Determinismo ───────────────────────────────────────────────────────

    #[test]
    fn estimation_is_deterministic() {
        let json = r#"{"a":1,"b":[true,null],"c":"texto"}"#;
        assert_eq!(estimate_json_tokens(json), estimate_json_tokens(json));
    }

    // ─── Estructura del JSON ────────────────────────────────────────────────

    #[test]
    fn counts_json_structure() {
        // {"a":1,"b":[true,null]}
        //  {        → 1
        //  "a"      → 3 (1 contenido + 2 comillas)
        //  :        → 1
        //  1        → 1
        //  ,        → 1
        //  "b"      → 3
        //  :        → 1
        //  [        → 1
        //  true     → 1
        //  ,        → 1
        //  null     → 1
        //  ]        → 1
        //  }        → 1
        assert_eq!(estimate_json_tokens(r#"{"a":1,"b":[true,null]}"#), 17);
    }

    #[test]
    fn whitespace_is_cheap_but_counted() {
        // One space collapses to ceil(1/4) = 1 extra token.
        assert_eq!(
            estimate_json_tokens(r#"{"a": 1}"#),
            estimate_json_tokens(r#"{"a":1}"#) + 1
        );
    }

    // ─── Escapes ────────────────────────────────────────────────────────────

    #[test]
    fn escaped_quotes_do_not_close_the_string() {
        // JSON string: "dice \"hola\""
        // Content:     dice "hola"  (11 chars) → ceil(11/4) = 3, + 2 quotes.
        assert_eq!(estimate_json_tokens(r#""dice \"hola\"""#), 5);
    }

    #[test]
    fn escaped_backslash_is_content() {
        // JSON string: "a\\b" → content `a\b` (3 chars) → ceil(3/4) = 1, + 2.
        assert_eq!(estimate_json_tokens(r#""a\\b""#), 3);
    }

    /// m2 — los escapes `\uXXXX` no se decodifican: la `u` y los cuatro dígitos
    /// entran como contenido literal, así que `\u00e9` se cobra como los 5
    /// caracteres ASCII `u00e9` (ceil(5/4) = 2) más las dos comillas → 4, en
    /// lugar de los 3 que costaría un `é` ya decodificado. Aproximación
    /// aceptada y deliberada (ver el comentario de la rama de escapes).
    #[test]
    fn unicode_escape_is_counted_verbatim_as_a_known_approximation() {
        assert_eq!(estimate_json_tokens(r#""\u00e9""#), 4);
    }

    // ─── Cadena vacía ───────────────────────────────────────────────────────

    #[test]
    fn empty_string_contributes_only_its_quotes() {
        // { → 1, "a" → 3, : → 1, "" → 0 + 2, } → 1
        assert_eq!(estimate_json_tokens(r#"{"a":""}"#), 8);
    }

    // ─── ASCII vs no-ASCII ──────────────────────────────────────────────────

    #[test]
    fn non_ascii_strings_cost_at_least_ascii() {
        let ascii = estimate_json_tokens(r#""aaaa""#);
        let non_ascii = estimate_json_tokens(r#""áááá""#);
        assert!(
            non_ascii >= ascii,
            "non-ASCII ({non_ascii}) must not cost less than ASCII ({ascii})"
        );
        assert!(
            non_ascii > ascii,
            "the ratio for non-ASCII must be denser than for ASCII"
        );
    }

    // ─── Literales y números ────────────────────────────────────────────────

    #[test]
    fn literals_and_numbers() {
        assert_eq!(estimate_literal_tokens("true"), 1);
        assert_eq!(estimate_literal_tokens("false"), 1);
        assert_eq!(estimate_literal_tokens("null"), 1);
        assert_eq!(estimate_literal_tokens("1"), 1);
        assert_eq!(estimate_literal_tokens("12"), 1);
        assert_eq!(estimate_literal_tokens("12345"), 2);
        assert_eq!(estimate_literal_tokens("1.5"), 1);
    }

    /// m4 — signo, decimal y exponente no se interpretan: el estimador solo
    /// cuenta dígitos (`ceil(dígitos / 2.5)`), así que `-1.5` y `15` colapsan
    /// a 1 y `1e10` (3 dígitos) sube a 2. Coherente con el enfoque heurístico;
    /// se fija la intención para que sea deliberado y no accidental.
    #[test]
    fn signed_decimal_and_exponent_numbers_use_the_digit_count() {
        assert_eq!(estimate_literal_tokens("-1.5"), 1);
        assert_eq!(estimate_literal_tokens("15"), 1);
        assert_eq!(estimate_literal_tokens("-3"), 1);
        assert_eq!(estimate_literal_tokens("0.0001"), 2);
        assert_eq!(estimate_literal_tokens("1e10"), 2);

        // Whole JSON: [ -1.5 , 15 , 1e10 ] → 1 + 1 + 1 + 1 + 1 + 2 + 1 = 8
        assert_eq!(estimate_json_tokens("[-1.5,15,1e10]"), 8);
    }

    // ─── Monotonía ──────────────────────────────────────────────────────────

    #[test]
    fn more_content_is_never_less() {
        let small = estimate_json_tokens(r#"{"a":"xxxxxxxx"}"#);
        let big = estimate_json_tokens(r#"{"a":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}"#);
        assert!(
            big >= small,
            "more content ({big}) must not estimate below less content ({small})"
        );

        let more_keys = estimate_json_tokens(
            r#"{"a":"xxxxxxxx","b":"yyyyyyyy","c":"zzzzzzzz","d":"wwwwwwww"}"#,
        );
        assert!(more_keys > small);
    }

    // ─── Calibración (reporte, no aserción) ─────────────────────────────────

    /// Reporte de calibración. No se ejecuta por defecto.
    ///
    /// Imprime la estimación de muestras JSON representativas frente al
    /// heurístico de markdown y a la cota `chars / 4`, y —si se define
    /// `CALIBRATION_DB` apuntando a un `valet.db`— resume los tokens reales que
    /// el proveedor guardó en `llm_requests`.
    ///
    /// Los tokens de `llm_requests` miden el prompt/respuesta completos
    /// (system prompt + herramientas + historial + estado inyectado), no un
    /// blob JSON aislado; por eso se imprimen como magnitud de referencia, no
    /// como comparación directa.
    ///
    /// Uso: `CALIBRATION_DB=valet.db cargo test token_estimate::tests::calibration_report -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "calibration report; run explicitly with --ignored --nocapture"]
    async fn calibration_report() {
        // A "typical" state: a profile with a dozen fields and a dozen rules.
        let mut typical_profile = serde_json::Map::new();
        for (k, v) in [
            ("name", "María José"),
            ("city", "Madrid"),
            ("country", "España"),
            ("language", "es"),
            ("occupation", "ingeniera"),
            ("timezone", "Europe/Madrid"),
            ("diet", "vegetariana"),
            ("pet", "gato llamado Lua"),
            ("hobby", "senderismo"),
            ("music", "jazz"),
            ("reading", "ciencia ficción"),
            ("coffee", "cortado sin azúcar"),
        ] {
            typical_profile.insert(k.to_string(), serde_json::Value::String(v.to_string()));
        }
        let typical_rules: Vec<serde_json::Value> = (0..12)
            .map(|i| {
                serde_json::Value::String(format!(
                    "Regla {i}: preferencia del usuario a recordar entre sesiones"
                ))
            })
            .collect();
        let typical = serde_json::to_string(&serde_json::json!({
            "schema_version": 1,
            "user_profile": typical_profile,
            "system_rules": typical_rules,
        }))
        .unwrap();

        // A "large" state: 120 rules, like the worker's big_state fixture.
        let large_rules: Vec<serde_json::Value> = (0..120)
            .map(|i| {
                serde_json::Value::String(format!(
                    "Regla número {i} con texto suficiente para inflar el recuento de tokens del estado persistente"
                ))
            })
            .collect();
        let large = serde_json::to_string(&serde_json::json!({
            "schema_version": 1,
            "user_profile": {"note": "perfil de prueba"},
            "system_rules": large_rules,
        }))
        .unwrap();

        let samples: [(&str, String); 5] = [
            (
                "estado mínimo",
                r#"{"schema_version":1,"user_profile":{"city":"Madrid"},"system_rules":["sé breve"]}"#
                    .to_string(),
            ),
            (
                "estado con no-ASCII",
                r#"{"schema_version":1,"user_profile":{"nombre":"María José"},"system_rules":["no inventes datos","responde en español"]}"#
                    .to_string(),
            ),
            ("estado típico (12+12)", typical),
            ("estado grande (120 reglas)", large),
            (
                "blob largo sin espacios",
                format!(
                    r#"{{"schema_version":1,"user_profile":{{"blob":"{}"}}}}"#,
                    "a".repeat(4000)
                ),
            ),
        ];

        println!("\n=== Calibración de estimate_json_tokens ===");
        for (name, json) in &samples {
            let estimate = estimate_json_tokens(json);
            let chars = json.chars().count();
            let lower_bound = chars / 4;
            let markdown = crate::models::message::estimate_markdown_tokens_heuristic(json);
            println!(
                "{name:>24} | estimator={estimate:>5} | chars/4={lower_bound:>5} | markdown={markdown:>5} | chars={chars}"
            );
        }

        let Ok(db_path) = std::env::var("CALIBRATION_DB") else {
            println!(
                "CALIBRATION_DB no definido: se omite el resumen de llm_requests.\n\
                 Define CALIBRATION_DB=<ruta valet.db> para incluir los tokens reales.\n"
            );
            return;
        };

        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&db_path)
            .read_only(true);
        let pool = match sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
        {
            Ok(pool) => pool,
            Err(err) => {
                println!("No se pudo abrir {db_path}: {err}");
                return;
            }
        };

        let row: Result<LlmRequestAggregates, _> = sqlx::query_as(
            "SELECT COUNT(*), AVG(prompt_tokens), AVG(completion_tokens), AVG(total_tokens) \
             FROM llm_requests WHERE status = 'success'",
        )
        .fetch_one(&pool)
        .await;

        match row {
            Ok((count, prompt, completion, total)) => println!(
                "llm_requests (success): n={count} | avg_prompt={:.1} | avg_completion={:.1} | avg_total={:.1}\n\
                 (tokens reales del proveedor sobre prompts completos, no JSON aislado)",
                prompt.unwrap_or(0.0),
                completion.unwrap_or(0.0),
                total.unwrap_or(0.0),
            ),
            Err(err) => println!("No se pudo leer llm_requests: {err}"),
        }
    }
}
