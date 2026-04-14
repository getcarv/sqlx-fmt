#![allow(clippy::format_in_format_args)]

use anyhow::{Result, bail};
use log::{debug, error};
use tree_sitter::{Node, Parser, Range};

pub fn format_query_macros_literals<F>(
    source: &str,
    literal_indentation: usize,
    macros_names: Vec<String>,
    mut formatter: F,
) -> String
where
    F: FnMut(&str, bool) -> Result<String>,
{
    // setup rust parser

    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("Error loading Rust grammar");
    let tree = parser
        .parse(source.as_bytes(), None)
        .expect("Failed to parse code");
    let root_node = tree.root_node();

    // find and collect replacements

    let mut replacements: Vec<(Range, String)> = Vec::new();

    find_and_collect(
        root_node,
        source.as_bytes(),
        &macros_names,
        literal_indentation,
        &mut formatter,
        &mut replacements,
    );

    // repace unformatted with formatted sql

    let mut result = source.to_string();
    for (range, replacement) in replacements.into_iter().rev() {
        let start = range.start_byte;
        let end = range.end_byte;
        result.replace_range(start..end, &replacement);
    }

    result
}

fn find_and_collect<'a, F>(
    node: Node<'a>,
    source: &'a [u8],
    macro_names: &Vec<String>,
    _literal_indentation: usize,
    formatter: &mut F,
    replacements: &mut Vec<(Range, String)>,
) where
    F: FnMut(&str, bool) -> Result<String>,
{
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "macro_invocation"
            && let Some(macro_node) = child.child_by_field_name("macro")
        {
            let macro_name = macro_node
                .utf8_text(source)
                .expect("failed to get macro name as utf8");
            if macro_names.contains(&macro_name.to_string())
                && let Some(sql_literal) = find_first_sql_literal(child)
            {
                let result = match sql_literal.kind() {
                    "raw_string_literal" => {
                        format_raw_string_literal(source, &sql_literal, formatter)
                    }
                    "string_literal" => format_string_literal(source, &sql_literal, formatter),
                    other => unreachable!("unexpected SQL literal kind: {other}"),
                };
                match result {
                    Ok(v) => replacements.push((sql_literal.range(), v)),
                    Err(e) => {
                        error!(
                            "failed to format SQL literal: {:?}, error: {:?}",
                            sql_literal.utf8_text(source),
                            e
                        );
                    }
                }
            }
        }
        find_and_collect(
            child,
            source,
            macro_names,
            _literal_indentation,
            formatter,
            replacements,
        );
    }
}

/// Find the SQL literal argument in a sqlx query macro invocation.
///
/// In every sqlx query macro the SQL is the first string-shaped positional
/// argument: either the first arg (`query!("SELECT...")`) or the second
/// (`query_as!(MyType, "SELECT...")`). It can be raw (`r#"..."#`) or regular
/// (`"..."`). Subsequent string literals are bound parameters and must not be
/// reformatted by the SQL formatter — doing so corrupts values like
/// `"user@example.com"` into `"user @ example.com"` because `@` is a Postgres
/// operator.
///
/// Returns the first literal of either kind in source order, or `None` when
/// the macro has no string literal arguments (e.g. `query!(query_string_var)`).
fn find_first_sql_literal(macro_invocation: Node) -> Option<Node> {
    let mut cursor = macro_invocation.walk();
    for macro_child in macro_invocation.children(&mut cursor) {
        let mut tt_cursor = macro_child.walk();
        for tt_child in macro_child.children(&mut tt_cursor) {
            if matches!(tt_child.kind(), "raw_string_literal" | "string_literal") {
                return Some(tt_child);
            }
        }
    }
    None
}

fn format_raw_string_literal<'a>(
    source: &'a [u8],
    raw_string_literal: &Node<'a>,
    formatter: &mut impl FnMut(&str, bool) -> Result<String>,
) -> Result<String> {
    let literal = raw_string_literal
        .utf8_text(source)
        .expect("failed to get raw string literal as utf8")
        .trim();

    let literal_text_lines_count = literal.lines().count();
    let (unquoted, hash_count) = unquote_raw_string_literal(literal);

    // Normalize indentation so the formatter always receives the same clean SQL.
    let clean_sql = strip_embedding_indent(unquoted);

    let col: usize = raw_string_literal.start_position().column;

    let formatter_res = formatter(&clean_sql, true);
    let Ok(replacement) = formatter_res else {
        bail!(
            "formatter failed to format sql {unquoted}, error: {:?}",
            formatter_res.err()
        );
    };

    let replacement_line_count = replacement.lines().count();

    debug!(
        "raw string literal => col: {col}, literal_lines: {literal_text_lines_count}, replacement_lines_count: {replacement_line_count}"
    );

    let new_literal = if literal_text_lines_count <= 1 && replacement_line_count > 1 {
        debug!("RAW_SINGLE_TO_MANY detected");
        format!(
            "{quote}{replacement}\n{unquote}",
            quote = format!("r{}\"\n", "#".repeat(hash_count)),
            replacement = replacement
                .lines()
                .map(|line| format!(
                    "{}{}",
                    if !line.trim().is_empty() {
                        " ".repeat(col)
                    } else {
                        "".to_string()
                    },
                    line
                ))
                .collect::<Vec<String>>()
                .join("\n")
                .trim_end(),
            unquote = format!("{}\"{}", " ".repeat(col), "#".repeat(hash_count))
        )
    } else if replacement.lines().count() <= 1 {
        debug!("RAW_SINGLE detected");
        format!(
            "{quote}{reappearance}{unquote}",
            quote = format!("r{}\"", "#".repeat(hash_count)),
            reappearance = replacement.trim(),
            unquote = format!("\"{}", "#".repeat(hash_count))
        )
    } else {
        debug!("RAW_MANY detected");
        format!(
            "{quote}{replacement}\n{unquote}",
            quote = format!("r{}\"\n", "#".repeat(hash_count)),
            replacement = replacement
                .lines()
                .map(|line| format!(
                    "{}{}",
                    if !line.trim().is_empty() {
                        " ".repeat(col)
                    } else {
                        "".to_string()
                    },
                    line
                ))
                .collect::<Vec<String>>()
                .join("\n")
                .trim_end(),
            unquote = format!("{}\"{}", " ".repeat(col), "#".repeat(hash_count))
        )
    };

    Ok(new_literal)
}

fn format_string_literal<'a>(
    source: &'a [u8],
    string_literal: &Node<'a>,
    formatter: &mut impl FnMut(&str, bool) -> Result<String>,
) -> Result<String> {
    let literal = string_literal
        .utf8_text(source)
        .expect("failed to get string literal as utf8")
        .trim();

    let literal_text_lines_count = literal.lines().count();
    let unquoted = &literal[1..literal.len() - 1];

    // Resolve Rust `\` line continuations and normalize indentation so the
    // formatter always receives the same clean SQL regardless of embedding.
    let clean_sql = resolve_line_continuations(unquoted);
    let clean_sql = strip_embedding_indent(&clean_sql);

    let col: usize = string_literal.start_position().column;

    let formatter_res = formatter(&clean_sql, true);
    let Ok(replacement) = formatter_res else {
        bail!(
            "formatter failed to format sql {unquoted}, error: {:?}",
            formatter_res.err()
        );
    };

    let replacement_line_count = replacement.lines().count();

    debug!(
        "string literal => col: {col}, literal_lines: {literal_text_lines_count}, replacement_lines_count: {replacement_line_count}"
    );

    let new_literal = if replacement_line_count <= 1 {
        // Single-line result: keep compact
        debug!("STRING_SINGLE detected");
        format!("\"{}\"", replacement.trim())
    } else if literal_text_lines_count <= 1 {
        // Original was single-line but formatted to multi-line
        debug!("STRING_SINGLE_TO_MANY detected");
        format!(
            "\"\n{replacement}\n{closing}\"",
            replacement = replacement
                .lines()
                .map(|line| format!(
                    "{}{}",
                    if !line.trim().is_empty() {
                        " ".repeat(col)
                    } else {
                        "".to_string()
                    },
                    line
                ))
                .collect::<Vec<String>>()
                .join("\n")
                .trim_end(),
            closing = " ".repeat(col),
        )
    } else {
        // Multi-line to multi-line: preserve multi-line structure
        debug!("STRING_MANY detected");
        format!(
            "\"\n{replacement}\n{closing}\"",
            replacement = replacement
                .lines()
                .map(|line| format!(
                    "{}{}",
                    if !line.trim().is_empty() {
                        " ".repeat(col)
                    } else {
                        "".to_string()
                    },
                    line
                ))
                .collect::<Vec<String>>()
                .join("\n")
                .trim_end(),
            closing = " ".repeat(col),
        )
    };

    Ok(new_literal)
}

fn unquote_raw_string_literal(lit: &str) -> (&str, usize) {
    // r#"..."#, r##"..."##, etc.
    let og_hashes = lit[1..].find('"').expect("invalid raw string literal");
    debug!("og_hashes: {og_hashes}");
    let hashes = &lit[1..=og_hashes];
    let content_start = og_hashes + 2; // 'r' + hashes + opening quote
    let content_end = lit.len() - (og_hashes + 1); // hashes + closing quote
    (&lit[content_start..content_end], hashes.len())
}

/// Convert Rust `\` line continuations to plain newlines.
///
/// In Rust string literals, a backslash followed by a newline (and optional
/// leading whitespace on the next line) is a line continuation that collapses
/// to nothing at runtime. Tree-sitter gives us raw source bytes, so we see
/// the `\` literally. Since `\` is not valid SQL, we convert these sequences
/// to newlines so the SQL formatter receives valid multi-line SQL.
fn resolve_line_continuations(text: &str) -> String {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut result = String::with_capacity(len);
    let mut i = 0;

    while i < len {
        if bytes[i] == b'\\' {
            // Check for line continuation: `\` followed by \r?\n
            let next = i + 1;
            if next < len && bytes[next] == b'\n' {
                // `\` + LF: skip both, skip leading whitespace, emit newline
                i = next + 1;
                while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                    i += 1;
                }
                result.push('\n');
                continue;
            } else if next + 1 < len && bytes[next] == b'\r' && bytes[next + 1] == b'\n' {
                // `\` + CRLF: skip all three, skip leading whitespace, emit newline
                i = next + 2;
                while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                    i += 1;
                }
                result.push('\n');
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }

    result
}

/// Strip embedding indentation from extracted SQL.
///
/// When SQL is embedded in a Rust string, each line after the first inherits
/// the indentation of the Rust source. This function removes the common
/// leading whitespace across all non-empty lines so the SQL formatter always
/// receives consistently-indented SQL, regardless of how deeply nested the
/// macro call is in Rust code.
fn strip_embedding_indent(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= 1 {
        return text.to_string();
    }

    // Find minimum indentation across non-empty lines (skip the first line
    // since it starts right after the opening quote and typically has no indent).
    let min_indent = lines
        .iter()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);

    if min_indent == 0 {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len());
    result.push_str(lines[0]);
    for line in &lines[1..] {
        result.push('\n');
        if line.len() >= min_indent && !line.trim().is_empty() {
            result.push_str(&line[min_indent..]);
        } else {
            result.push_str(line);
        }
    }

    result
}
