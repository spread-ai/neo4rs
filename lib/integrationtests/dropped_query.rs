use std::time::Duration;

use neo4rs::*;

mod container;

/// A query whose future is dropped while it waits on the server leaves that query's responses
/// unread on its connection. The pool must not hand that connection to the next query.
#[tokio::test]
async fn next_query_after_a_dropped_one_reads_its_own_responses() {
    let config = ConfigBuilder::default().max_connections(1);
    let neo4j = container::Neo4jContainer::from_config(config).await;
    let graph = neo4j.graph();

    graph.run(query("RETURN 1")).await.unwrap();

    // Slow to plan (and new to the plan cache), so the drop lands while `execute` waits for the
    // RUN reply.
    let run = std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .unwrap()
        .as_nanos();
    let slow_to_plan = (0..300)
        .map(|i| format!("MATCH (n:DroppedQuery{i}) WHERE n.p{i} = {run} RETURN count(n) AS n"))
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let dropped = tokio::time::timeout(
        Duration::from_millis(20),
        graph.execute(query(&slow_to_plan)),
    )
    .await;
    assert!(
        dropped.is_err(),
        "the slow query was planned before it was dropped"
    );

    // Let the server finish the dropped query, so its responses wait on the connection.
    tokio::time::sleep(Duration::from_secs(5)).await;

    let n = graph
        .execute(query("RETURN 42 AS n"))
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get::<i64>("n")
        .unwrap();
    assert_eq!(n, 42);
}
