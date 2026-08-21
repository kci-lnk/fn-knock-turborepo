use super::*;

#[tokio::test]
async fn stream_ids_follow_numeric_order_and_remain_monotonic() {
    let mut conn = temp_manager().await;
    for id in ["9-0", "10-0"] {
        let inserted: String = cmd("XADD")
            .arg("fn_knock:test:numeric-stream")
            .arg(id)
            .arg("value")
            .arg(id)
            .query_async(&mut conn)
            .await
            .expect("insert explicit stream ID");
        assert_eq!(inserted, id);
    }

    let ascending: Vec<(String, Vec<String>)> = cmd("XRANGE")
        .arg("fn_knock:test:numeric-stream")
        .arg("-")
        .arg("+")
        .query_async(&mut conn)
        .await
        .expect("numeric xrange");
    assert_eq!(
        ascending
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        vec!["9-0", "10-0"]
    );

    let read = conn
        .xread_options(
            &["fn_knock:test:numeric-stream"],
            &["0-0"],
            &streams::StreamReadOptions::default().count(10),
        )
        .await
        .expect("numeric xread")
        .expect("xread rows");
    assert_eq!(
        read.keys[0]
            .ids
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["9-0", "10-0"]
    );

    let descending = conn
        .xrevrange_count("fn_knock:test:numeric-stream", "+", "-", 10)
        .await
        .expect("numeric xrevrange");
    assert_eq!(
        descending
            .ids
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["10-0", "9-0"]
    );

    let _: () = cmd("XTRIM")
        .arg("fn_knock:test:numeric-stream")
        .arg("MINID")
        .arg("~")
        .arg("10-0")
        .query_async(&mut conn)
        .await
        .expect("numeric xtrim");
    let remaining: Vec<(String, Vec<String>)> = cmd("XRANGE")
        .arg("fn_knock:test:numeric-stream")
        .arg("-")
        .arg("+")
        .query_async(&mut conn)
        .await
        .expect("remaining stream entries");
    assert_eq!(remaining[0].0, "10-0");

    let _: () = cmd("XDEL")
        .arg("fn_knock:test:numeric-stream")
        .arg("10-0")
        .query_async(&mut conn)
        .await
        .expect("empty stream");
    assert_eq!(
        conn.exists("fn_knock:test:numeric-stream").await.unwrap(),
        1
    );

    let generated: String = cmd("XADD")
        .arg("fn_knock:test:numeric-stream")
        .arg("*")
        .arg("value")
        .arg("generated")
        .query_async(&mut conn)
        .await
        .expect("generate monotonic ID");
    assert!(parse_stream_id(&generated).unwrap() > parse_stream_id("10-0").unwrap());

    let duplicate = cmd("XADD")
        .arg("fn_knock:test:numeric-stream")
        .arg("10-0")
        .arg("value")
        .arg("duplicate")
        .query_async::<String>(&mut conn)
        .await;
    assert!(duplicate.is_err());

    let future_ms = now_ms() + 60_000;
    let future_id = format!("{future_ms}-0");
    let _: String = cmd("XADD")
        .arg("fn_knock:test:clock-rollback-stream")
        .arg(&future_id)
        .arg("value")
        .arg("future")
        .query_async(&mut conn)
        .await
        .expect("seed future stream ID");
    let _: () = cmd("XDEL")
        .arg("fn_knock:test:clock-rollback-stream")
        .arg(&future_id)
        .query_async(&mut conn)
        .await
        .expect("remove future stream entry");
    let after_rollback: String = cmd("XADD")
        .arg("fn_knock:test:clock-rollback-stream")
        .arg("*")
        .arg("value")
        .arg("after-rollback")
        .query_async(&mut conn)
        .await
        .expect("generate after clock rollback");
    assert_eq!(after_rollback, format!("{future_ms}-1"));
}

#[tokio::test]
async fn xtrim_maxlen_keeps_newest_entries() {
    let mut conn = temp_manager().await;
    for id in ["1-0", "2-0", "3-0"] {
        let _: String = cmd("XADD")
            .arg("fn_knock:test:maxlen-stream")
            .arg(id)
            .arg("value")
            .arg(id)
            .query_async(&mut conn)
            .await
            .expect("seed stream");
    }
    let _: () = cmd("XTRIM")
        .arg("fn_knock:test:maxlen-stream")
        .arg("MAXLEN")
        .arg("~")
        .arg(2)
        .query_async(&mut conn)
        .await
        .expect("trim stream length");
    let remaining: Vec<(String, Vec<String>)> = cmd("XRANGE")
        .arg("fn_knock:test:maxlen-stream")
        .arg("-")
        .arg("+")
        .query_async(&mut conn)
        .await
        .expect("read trimmed stream");
    assert_eq!(
        remaining
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        vec!["2-0", "3-0"]
    );
}
