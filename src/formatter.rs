use anyhow::{Result, bail};
use log::info;
use std::io::Write;
use std::process::{Command, Stdio};

pub fn sqruff(content: &str, config: &str) -> Result<String> {
    let config_exits = std::path::Path::new(config).exists();
    let mut child = if config_exits {
        Command::new("sqruff")
            .arg("--config")
            .arg(config)
            .arg("fix")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?
    } else {
        info!("sqruff config file not found at {config}, using default sqruff config");
        Command::new("sqruff")
            .arg("fix")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?
    };

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(content.trim().as_bytes())?;
    }
    let output = child.wait_with_output()?;
    let formatted = String::from_utf8_lossy(&output.stdout);

    if formatted.trim().is_empty() {
        bail!(
            "failed to format sql, error: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let formatted = normalize_formatted_sql(formatted.trim_end());
    let formatted = format!("{formatted}\n");

    Ok(formatted.to_string())
}

fn normalize_formatted_sql(sql: &str) -> String {
    let mut normalized = Vec::new();
    let lines: Vec<&str> = sql.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        if line.trim_end().ends_with(" ON")
            && let Some(next_line) = lines.get(i + 1)
            && should_inline_on_condition(next_line.trim())
        {
            normalized.push(format!("{} {}", line.trim_end(), next_line.trim()));
            i += 2;
            continue;
        }

        normalized.push(line.to_string());
        i += 1;
    }

    normalized.join("\n")
}

fn should_inline_on_condition(next_line: &str) -> bool {
    if next_line.is_empty() {
        return false;
    }

    if next_line.starts_with("AND ") || next_line.starts_with("OR ") {
        return false;
    }

    !starts_new_clause(next_line)
}

fn starts_new_clause(line: &str) -> bool {
    const CLAUSE_PREFIXES: &[&str] = &[
        "SELECT",
        "FROM",
        "WHERE",
        "GROUP",
        "ORDER",
        "HAVING",
        "LIMIT",
        "OFFSET",
        "QUALIFY",
        "WINDOW",
        "JOIN",
        "LEFT",
        "RIGHT",
        "INNER",
        "FULL",
        "CROSS",
        "UNION",
        "EXCEPT",
        "INTERSECT",
        "RETURNING",
        "VALUES",
        "SET",
        "ON",
        "USING",
    ];

    CLAUSE_PREFIXES
        .iter()
        .any(|prefix| line == *prefix || line.strip_prefix(prefix).is_some_and(|rest| rest.starts_with(' ')))
}
