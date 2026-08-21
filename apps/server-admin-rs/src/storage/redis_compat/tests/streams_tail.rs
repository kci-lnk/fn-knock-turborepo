use super::*;

#[tokio::test]
async fn xread_uses_stream_id_order_when_cursor_entry_was_deleted() {
    let mut conn = temp_manager().await;
    for id in ["1-0", "2-0", "3-0"] {
        let _: String = cmd("XADD")
            .arg("fn_knock:test:stream-deleted-cursor")
            .arg(id)
            .arg("value")
            .arg(id)
            .query_async(&mut conn)
            .await
            .expect("xadd");
    }
    let _: () = cmd("XDEL")
        .arg("fn_knock:test:stream-deleted-cursor")
        .arg("2-0")
        .query_async(&mut conn)
        .await
        .expect("xdel cursor row");

    let reply = conn
        .xread_options(
            &["fn_knock:test:stream-deleted-cursor"],
            &["2-0"],
            &streams::StreamReadOptions::default().count(10),
        )
        .await
        .expect("xread")
        .expect("reply");

    let ids = reply.keys[0]
        .ids
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["3-0"]);
}

#[tokio::test]
async fn stream_entries_preserve_field_order_and_duplicates() {
    let mut conn = temp_manager().await;
    let _: String = cmd("XADD")
        .arg("fn_knock:test:stream-order")
        .arg("1-0")
        .arg("z")
        .arg("last")
        .arg("a")
        .arg("first")
        .arg("z")
        .arg("again")
        .query_async(&mut conn)
        .await
        .expect("xadd ordered fields");

    let entries: Vec<(String, Vec<String>)> = cmd("XRANGE")
        .arg("fn_knock:test:stream-order")
        .arg("-")
        .arg("+")
        .query_async(&mut conn)
        .await
        .expect("xrange ordered fields");

    assert_eq!(
        entries,
        vec![(
            "1-0".to_string(),
            vec![
                "z".to_string(),
                "last".to_string(),
                "a".to_string(),
                "first".to_string(),
                "z".to_string(),
                "again".to_string(),
            ],
        )]
    );
}
