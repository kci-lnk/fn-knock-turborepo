use super::*;

#[tokio::test]
async fn set_clears_existing_ttl() {
    let mut conn = temp_manager().await;
    conn.set_ex("fn_knock:test:string", "old", 60)
        .await
        .expect("set expiring value");
    assert!(conn.ttl("fn_knock:test:string").await.expect("read ttl") > 0);

    conn.set("fn_knock:test:string", "new")
        .await
        .expect("overwrite value");
    let value: Option<String> = conn.get("fn_knock:test:string").await.expect("read value");
    assert_eq!(value.as_deref(), Some("new"));
    assert_eq!(
        conn.ttl("fn_knock:test:string")
            .await
            .expect("read cleared ttl"),
        -1
    );
}

#[tokio::test]
async fn sorted_set_score_bounds_follow_redis_exclusive_syntax() {
    let mut conn = temp_manager().await;
    conn.zadd("fn_knock:test:zset-bounds", "a", 10)
        .await
        .expect("zadd a");
    conn.zadd("fn_knock:test:zset-bounds", "b", 20)
        .await
        .expect("zadd b");
    conn.zadd("fn_knock:test:zset-bounds", "c", 30)
        .await
        .expect("zadd c");

    let count: i64 = cmd("ZCOUNT")
        .arg("fn_knock:test:zset-bounds")
        .arg("10")
        .arg("(30")
        .query_async(&mut conn)
        .await
        .expect("zcount exclusive max");
    assert_eq!(count, 2);

    let members: Vec<String> = cmd("ZRANGEBYSCORE")
        .arg("fn_knock:test:zset-bounds")
        .arg("(10")
        .arg("30")
        .query_async(&mut conn)
        .await
        .expect("zrangebyscore exclusive min");
    assert_eq!(members, vec!["b".to_string(), "c".to_string()]);

    let reverse_pairs: Vec<String> = cmd("ZREVRANGEBYSCORE")
        .arg("fn_knock:test:zset-bounds")
        .arg("+inf")
        .arg("(10")
        .arg("WITHSCORES")
        .arg("LIMIT")
        .arg(0)
        .arg(2)
        .query_async(&mut conn)
        .await
        .expect("zrevrangebyscore reverse score bounds");
    assert_eq!(
        reverse_pairs,
        vec![
            "c".to_string(),
            "30".to_string(),
            "b".to_string(),
            "20".to_string(),
        ]
    );

    let _: () = cmd("ZREMRANGEBYSCORE")
        .arg("fn_knock:test:zset-bounds")
        .arg("-inf")
        .arg("(20")
        .query_async(&mut conn)
        .await
        .expect("zremrangebyscore exclusive max");
    assert_eq!(
        conn.zrange("fn_knock:test:zset-bounds", 0, -1)
            .await
            .expect("zrange remaining"),
        vec!["b".to_string(), "c".to_string()]
    );
}

#[tokio::test]
async fn ranges_outside_collection_bounds_are_empty() {
    let mut conn = temp_manager().await;
    let _: () = cmd("RPUSH")
        .arg("fn_knock:test:range-list")
        .arg(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        .query_async(&mut conn)
        .await
        .expect("seed list");
    for (member, score) in [("a", 1), ("b", 2), ("c", 3)] {
        conn.zadd("fn_knock:test:range-zset", member, score)
            .await
            .expect("seed zset");
    }

    assert!(
        conn.lrange("fn_knock:test:range-list", 3, -1)
            .await
            .expect("list start at length")
            .is_empty()
    );
    assert!(
        conn.zrange("fn_knock:test:range-zset", 4, 10)
            .await
            .expect("zset start beyond length")
            .is_empty()
    );
    assert!(
        conn.lrange("fn_knock:test:range-list", 0, -4)
            .await
            .expect("list end before first item")
            .is_empty()
    );
    assert_eq!(
        conn.zrange("fn_knock:test:range-zset", -100, -1)
            .await
            .expect("large negative start"),
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
}

#[tokio::test]
async fn empty_collections_remove_their_redis_keys() {
    let mut conn = temp_manager().await;

    conn.hset("fn_knock:test:empty-hash", "field", "value")
        .await
        .expect("seed hash");
    conn.hdel("fn_knock:test:empty-hash", "field")
        .await
        .expect("empty hash");
    assert_eq!(conn.exists("fn_knock:test:empty-hash").await.unwrap(), 0);

    conn.sadd("fn_knock:test:empty-set", "member")
        .await
        .expect("seed set");
    conn.srem("fn_knock:test:empty-set", "member")
        .await
        .expect("empty set");
    assert_eq!(conn.exists("fn_knock:test:empty-set").await.unwrap(), 0);

    conn.zadd("fn_knock:test:empty-zset", "member", 1)
        .await
        .expect("seed zset");
    let _: () = cmd("ZREMRANGEBYSCORE")
        .arg("fn_knock:test:empty-zset")
        .arg("-inf")
        .arg("+inf")
        .query_async(&mut conn)
        .await
        .expect("empty zset");
    assert_eq!(conn.exists("fn_knock:test:empty-zset").await.unwrap(), 0);

    let _: () = cmd("RPUSH")
        .arg("fn_knock:test:empty-list")
        .arg("value")
        .query_async(&mut conn)
        .await
        .expect("seed list");
    let _: () = cmd("LTRIM")
        .arg("fn_knock:test:empty-list")
        .arg(1)
        .arg(0)
        .query_async(&mut conn)
        .await
        .expect("empty list");
    assert_eq!(conn.exists("fn_knock:test:empty-list").await.unwrap(), 0);
}
