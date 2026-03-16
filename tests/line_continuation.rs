mod common;

/// Test that `\` line continuations in regular string literals are handled correctly.
/// This is the main convergence fix: without it, sqruff receives invalid SQL containing
/// literal `\` characters, producing different output on each pass.
#[test_log::test]
fn line_continuation() {
    let content = r###"
    sqlx::query!("SELECT * \
        FROM test \
        WHERE id = $1")
    "###;

    // After formatting, the `\` continuations should be resolved into a proper
    // multi-line string with correct indentation.
    let formatted = sqlx_fmt::format(content, ".sqruff", 4, &None).unwrap();

    // The result should NOT contain any `\` line continuations
    assert!(
        !formatted.contains("\\\n"),
        "formatted output still contains \\ line continuations:\n{formatted}"
    );

    // The result should contain valid SQL keywords
    assert!(formatted.contains("SELECT"));
    assert!(formatted.contains("FROM"));
    assert!(formatted.contains("WHERE"));
}

/// Test that formatting is idempotent: format(format(x)) == format(x).
/// This is the convergence property required for `sqlx-fmt check` to work.
#[test_log::test]
fn line_continuation_idempotent() {
    let content = r###"
    sqlx::query!("SELECT * \
        FROM test \
        WHERE id = $1")
    "###;

    let pass1 = sqlx_fmt::format(content, ".sqruff", 4, &None).unwrap();
    let pass2 = sqlx_fmt::format(&pass1, ".sqruff", 4, &None).unwrap();

    assert_eq!(
        pass1, pass2,
        "formatting is not idempotent!\npass1:\n{pass1}\npass2:\n{pass2}"
    );
}

/// Test idempotency for already-formatted multi-line strings (no `\`).
#[test_log::test]
fn multiline_string_idempotent() {
    let content = "    sqlx::query!(\"\n        SELECT *\n        FROM test\n        WHERE id = $1\n        \")\n";

    let pass1 = sqlx_fmt::format(content, ".sqruff", 4, &None).unwrap();
    let pass2 = sqlx_fmt::format(&pass1, ".sqruff", 4, &None).unwrap();

    assert_eq!(
        pass1, pass2,
        "multi-line formatting is not idempotent!\npass1:\n{pass1}\npass2:\n{pass2}"
    );
}
