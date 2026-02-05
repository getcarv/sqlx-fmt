mod common;

// do not alter the content of the migrate! macro
#[test_log::test]
fn migrate() {
    let content = r###"
        sqlx::migrate!("./migrations")
    "###;

    let formatted = sqlx_fmt::format(content, ".sqruff", 4, &None).unwrap();

    let expected = r###"
        sqlx::migrate!("./migrations")
    "###;

    common::compare(expected, &formatted);
}
