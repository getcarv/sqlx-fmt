mod common;

// Regression: SQL formatter must not touch string literal parameters that
// follow the SQL template. Earlier versions formatted the first string_literal
// in addition to the SQL, corrupting values like "user@carv.ai" into
// "user @ carv.ai" because `@` is a Postgres operator.
#[test_log::test]
fn raw_sql_with_string_literal_params_is_preserved() {
    let content = r###"
    sqlx::query!(
        r#"
        select *   from carv.profile_view where email = $1
        "#,
        "user@carv.ai"
    )
    "###;

    let expected = r###"
    sqlx::query!(
        r#"
        select * from carv.profile_view
        where email = $1
        "#,
        "user@carv.ai"
    )
    "###;

    let formatted = sqlx_fmt::format(content, ".sqruff", 4, &None).unwrap();
    common::compare(expected, &formatted);
}

#[test_log::test]
fn raw_sql_with_hyphenated_string_literal_param_is_preserved() {
    let content = r###"
    sqlx::query!(
        r#"insert into carv.discount (title) values ($1)"#,
        "PRODEAL-TEST"
    )
    "###;

    let expected = r###"
    sqlx::query!(
        r#"insert into carv.discount (title) values ($1)"#,
        "PRODEAL-TEST"
    )
    "###;

    let formatted = sqlx_fmt::format(content, ".sqruff", 4, &None).unwrap();
    common::compare(expected, &formatted);
}

#[test_log::test]
fn query_as_with_raw_sql_and_multiple_string_literal_params_is_preserved() {
    let content = r###"
    sqlx::query_as!(
        User,
        r#"select id from carv.user where email = $1 and code = $2"#,
        "user@carv.ai",
        "PRODEAL-TEST"
    )
    "###;

    let expected = r###"
    sqlx::query_as!(
        User,
        r#"select id from carv.user where email = $1 and code = $2"#,
        "user@carv.ai",
        "PRODEAL-TEST"
    )
    "###;

    let formatted = sqlx_fmt::format(content, ".sqruff", 4, &None).unwrap();
    common::compare(expected, &formatted);
}

// SQL passed as a regular (non-raw) string is still formatted, even when
// followed by string-literal parameters.
#[test_log::test]
fn regular_string_sql_is_formatted_string_literal_params_are_preserved() {
    let content = r###"
        sqlx::query!("select id   from carv.user where email = $1", "user@carv.ai")
    "###;

    let expected = r###"
        sqlx::query!("select id from carv.user where email = $1", "user@carv.ai")
    "###;

    let formatted = sqlx_fmt::format(content, ".sqruff", 4, &None).unwrap();
    common::compare(expected, &formatted);
}
