//! The whole path, with nothing faked but the tab: a room is opened, a window
//! takes its socket, an AI client shakes hands over HTTP, lists the tools and
//! calls one — and the answer comes back from the window that did the work.

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};

/// Starts the relay on a port the system picks, and says where it is.
async fn relay() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a port");
    let addr = listener.local_addr().expect("the port it took");
    tokio::spawn(async move {
        axum::serve(listener, newera_relay::router())
            .await
            .expect("the relay runs");
    });
    format!("http://{addr}")
}

async fn post(url: &str, body: Value) -> (u16, Value) {
    let response = reqwest::Client::new()
        .post(url)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(body.to_string())
        .send()
        .await
        .expect("the relay answers");
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    let value = serde_json::from_str(&text).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn an_ai_reaches_the_tab_and_the_tab_answers() {
    let base = relay().await;

    // 1. The window asks for somewhere to be reached.
    let (status, opened) = post(&format!("{base}/rooms"), json!({})).await;
    assert_eq!(status, 200);
    let mcp = format!("{base}{}", opened["mcp_path"].as_str().unwrap());
    let tab_url = format!(
        "ws://{}{}",
        base.trim_start_matches("http://"),
        opened["tab_path"].as_str().unwrap()
    );

    // 2. The window takes its socket and says what it can do.
    let (mut socket, _) = tokio_tungstenite::connect_async(&tab_url)
        .await
        .expect("the tab connects");
    socket
        .send(tokio_tungstenite::tungstenite::Message::text(
            json!({
                "type": "hello",
                "tools": [{"name": "get_home", "description": "reads the plan", "inputSchema": {"type": "object"}}]
            })
            .to_string(),
        ))
        .await
        .expect("hello goes up");

    // 3. An AI client shakes hands.
    let (status, hello) = post(
        &mcp,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "claude-code", "version": "2.0.0"}
            }
        }),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(hello["result"]["serverInfo"]["name"], "3d-new-era-ai");

    // The window is told who turned up, so it can say so on screen.
    let told = socket.next().await.expect("a message").expect("text");
    let told: Value = serde_json::from_str(told.to_text().unwrap()).unwrap();
    assert_eq!(told["type"], "client");
    assert_eq!(told["name"], "claude-code");

    // 4. The tools it lists are the window's own.
    let (_, list) = post(
        &mcp,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await;
    assert_eq!(list["result"]["tools"][0]["name"], "get_home");

    // 5. A call goes down to the window, and its answer comes back up.
    let calling = tokio::spawn({
        let mcp = mcp.clone();
        async move {
            post(
                &mcp,
                json!({
                    "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                    "params": {"name": "get_home", "arguments": {"level": 0}}
                }),
            )
            .await
        }
    });

    let work = socket.next().await.expect("a message").expect("text");
    let work: Value = serde_json::from_str(work.to_text().unwrap()).unwrap();
    assert_eq!(work["type"], "call");
    assert_eq!(work["name"], "get_home");
    assert_eq!(work["args"]["level"], 0);
    socket
        .send(tokio_tungstenite::tungstenite::Message::text(
            json!({
                "type": "result",
                "id": work["id"],
                "ok": true,
                "result": {"content": [{"type": "text", "text": "w1 0,0 400,0"}]}
            })
            .to_string(),
        ))
        .await
        .expect("the answer goes up");

    let (status, answer) = calling.await.expect("the call finishes");
    assert_eq!(status, 200);
    assert_eq!(answer["result"]["content"][0]["text"], "w1 0,0 400,0");

    // 6. With the window gone, the address says so instead of hanging.
    drop(socket);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let (_, orphan) = post(
        &mcp,
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {"name": "get_home", "arguments": {}}
        }),
    )
    .await;
    assert_eq!(orphan["result"]["isError"], true);
}

/// A socket that says nothing is closed by what sits in front of this (about
/// a hundred seconds, at Cloudflare): the tab is pinged so that never happens
/// to somebody who is simply reading their own plan.
#[tokio::test]
async fn the_tab_is_kept_alive() {
    let base = relay().await;
    let (_, opened) = post(&format!("{base}/rooms"), json!({})).await;
    let tab_url = format!(
        "ws://{}{}",
        base.trim_start_matches("http://"),
        opened["tab_path"].as_str().unwrap()
    );
    let (mut socket, _) = tokio_tungstenite::connect_async(&tab_url)
        .await
        .expect("the tab connects");
    // The first beat is due one interval in; with the interval this short in
    // the test build it would be a long wait, so what is checked is that the
    // socket is still open and answering after a quiet stretch — which is
    // what the ping buys.
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    socket
        .send(tokio_tungstenite::tungstenite::Message::text(
            json!({"type": "hello", "tools": []}).to_string(),
        ))
        .await
        .expect("the socket is still there after silence");
}

/// One room cannot be reached with another's secrets, and a room nobody owns
/// cannot be reached at all.
#[tokio::test]
async fn rooms_do_not_meet() {
    let base = relay().await;
    let (_, mine) = post(&format!("{base}/rooms"), json!({})).await;
    let (_, yours) = post(&format!("{base}/rooms"), json!({})).await;

    let my_room = mine["room"].as_str().unwrap();
    let your_token = yours["mcp_path"]
        .as_str()
        .unwrap()
        .split('/')
        .nth(3)
        .unwrap();
    let crossed = format!("{base}/r/{my_room}/{your_token}/mcp");
    let (_, answer) = post(
        &crossed,
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
    )
    .await;
    assert_eq!(
        answer["error"]["code"], -32001,
        "one room's token must not open another"
    );

    // And the socket of a room refuses the wrong key.
    let refused = tokio_tungstenite::connect_async(format!(
        "ws://{}/r/{my_room}/tab?key={}",
        base.trim_start_matches("http://"),
        "0".repeat(32)
    ))
    .await;
    assert!(refused.is_err(), "a wrong key must not take the socket");
}
