mod common;

#[test_log::test]
fn sqlite_where_clause_stays_indented() {
    let content = r###"
    sqlx::query!(
        "
        SELECT
            value
        FROM
            cache
        WHERE
        key = $1 ;
        "
    )
    "###;

    let expected = r###"
    sqlx::query!(
        "
        SELECT
            value
        FROM
            cache
        WHERE
            key = $1;
        "
    )
    "###;

    let formatted = sqlx_fmt::format(content, ".sqruff", 4, &None).unwrap();
    common::compare(expected, &formatted);
}
