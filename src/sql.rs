#[derive(Clone, Debug)]
pub struct SqlPlan {
    pub statements: Vec<String>,
    pub read_only_candidate: bool,
}

pub fn analyze(sql: &str) -> Result<SqlPlan, String> {
    let mut statements = Vec::new();
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut start = 0;
    let mut quote = None;
    let mut line_comment = false;
    let mut block_comment = false;
    let mut executable_comment = false;
    let mut has_code = false;
    let mut safe = true;
    let chars: Vec<(usize, char)> = sql.char_indices().collect();
    let mut index = 0;
    let flush = |token: &mut String, tokens: &mut Vec<String>| {
        if !token.is_empty() {
            tokens.push(std::mem::take(token).to_ascii_uppercase());
        }
    };
    while index < chars.len() {
        let (offset, ch) = chars[index];
        let next = chars.get(index + 1).map(|(_, ch)| *ch);
        if line_comment {
            if ch == '\n' {
                line_comment = false;
            }
        } else if block_comment {
            if ch == '*' && next == Some('/') {
                block_comment = false;
                index += 1;
            }
        } else if let Some(delimiter) = quote {
            if ch == '\\' {
                index += 1;
            } else if ch == delimiter {
                if next == Some(delimiter) {
                    index += 1;
                } else {
                    quote = None;
                }
            }
        } else if ch == '#'
            || (ch == '-'
                && next == Some('-')
                && chars
                    .get(index + 2)
                    .is_none_or(|(_, ch)| ch.is_whitespace()))
        {
            flush(&mut token, &mut tokens);
            line_comment = true;
        } else if ch == '/' && next == Some('*') {
            flush(&mut token, &mut tokens);
            executable_comment |= chars.get(index + 2).is_some_and(|(_, ch)| *ch == '!');
            block_comment = true;
            index += 1;
        } else if matches!(ch, '\'' | '"' | '`') {
            flush(&mut token, &mut tokens);
            quote = Some(ch);
            has_code = true;
        } else if ch == ';' {
            flush(&mut token, &mut tokens);
            if has_code {
                safe &= read_only_tokens(&tokens) && !executable_comment;
                statements.push(sql[start..offset].trim().to_owned());
            }
            start = offset + 1;
            has_code = false;
            executable_comment = false;
            tokens.clear();
        } else if ch.is_ascii_alphanumeric() || ch == '_' {
            token.push(ch);
            has_code = true;
        } else {
            flush(&mut token, &mut tokens);
            has_code |= !ch.is_whitespace();
        }
        index += 1;
    }
    if quote.is_some() || block_comment {
        return Err("SQL incompleto: feche as aspas ou o comentário.".into());
    }
    flush(&mut token, &mut tokens);
    if has_code || executable_comment {
        safe &= read_only_tokens(&tokens) && !executable_comment;
        statements.push(sql[start..].trim().to_owned());
    }
    if statements.is_empty() {
        return Err("Escreva uma consulta SQL antes de executar.".into());
    }
    if statements.iter().any(|statement| {
        statement
            .trim_start()
            .to_ascii_uppercase()
            .starts_with("DELIMITER")
    }) {
        return Err("DELIMITER é um comando de cliente. Envie a definição da rotina como uma instrução separada.".into());
    }
    if statements.len() > 100 {
        return Err(
            "Um lote pode conter no máximo 100 instruções. Divida o script em lotes menores."
                .into(),
        );
    }
    Ok(SqlPlan {
        statements,
        read_only_candidate: safe,
    })
}

fn read_only_tokens(tokens: &[String]) -> bool {
    matches!(
        tokens.first().map(String::as_str),
        Some("SELECT" | "SHOW" | "DESC" | "DESCRIBE" | "EXPLAIN")
    ) && !tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "OUTFILE" | "DUMPFILE" | "GET_LOCK" | "RELEASE_LOCK"
        )
    })
}

pub fn may_contain_credentials(sql: &str) -> bool {
    let upper = sql.to_ascii_uppercase();
    ["PASSWORD", "IDENTIFIED", "SECRET", "TOKEN", "SENHA"]
        .iter()
        .any(|word| upper.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strings_and_comments_do_not_split_statements() {
        let plan =
            analyze("-- update ignored\nSELECT 'a;b', 'it\\\'s'; /* ; */ SELECT `a;b` FROM t;")
                .unwrap();
        assert_eq!(plan.statements.len(), 2);
        assert!(plan.read_only_candidate);
    }
    #[test]
    fn writes_and_executable_comments_are_conservative() {
        for sql in [
            "SELECT 1; DELETE FROM t",
            "/*!50000 DELETE FROM t */",
            "SELECT 1 INTO OUTFILE 'x'",
            "WITH x AS (SELECT 1) SELECT * FROM x",
        ] {
            assert!(!analyze(sql).unwrap().read_only_candidate, "{sql}");
        }
        assert!(analyze("SELECT 'OUTFILE'").unwrap().read_only_candidate);
    }
    #[test]
    fn rejects_incomplete_and_empty_sql() {
        for sql in ["", "-- hi", "SELECT 'a", "/* no end"] {
            assert!(analyze(sql).is_err());
        }
    }
}
