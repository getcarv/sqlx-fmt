mod common;

#[test_log::test]
fn join_on_keeps_first_condition_inline() {
    let content = r###"
    sqlx::query!(
        "
        SELECT
            stores.domain
        FROM
            carv.membership
            JOIN shopify.product_variants ON product_variants.sku = sku.title
            JOIN shopify.products ON
                products.id = product_variants.product_id
                AND products.store_uuid = stores.uuid
        WHERE
            users.email = $1
        "
    )
    "###;

    let expected = r###"
    sqlx::query!(
        "
        SELECT
            stores.domain
        FROM
            carv.membership
            JOIN shopify.product_variants ON product_variants.sku = sku.title
            JOIN shopify.products ON products.id = product_variants.product_id
                AND products.store_uuid = stores.uuid
        WHERE
            users.email = $1
        "
    )
    "###;

    let formatted = sqlx_fmt::format(content, ".sqruff", 4, &None).unwrap();
    common::compare(expected, &formatted);
}
