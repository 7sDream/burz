use std::time::Duration;

use burz::ws::event::EventData;
use burz::ws::message::{Hello, Message, OnlyData};
use burz::ws::Event;
use burz::Bot;

use futures_util::{future, SinkExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite as websocket;

use tokio_tungstenite::WebSocketStream;

async fn fake_gateway_no_pong_process(mut conn: WebSocketStream<TcpStream>) {
    let hello = Message::Hello(OnlyData {
        data: Hello {
            code: 0,
            session_id: Some("x".to_string()),
        },
    });
    conn.feed(websocket::Message::Binary(hello.encode()))
        .await
        .unwrap();

    let event = Message::Event(EventData {
        sn: 1234,
        event: Event::Null,
    });

    conn.feed(websocket::Message::Binary(event.encode()))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(40)).await;

    conn.feed(websocket::Message::Binary(Message::Pong.encode()))
        .await
        .unwrap();

    future::pending().await
}

async fn fake_gateway_no_pong() {
    let listener = TcpListener::bind("127.0.0.1:7777").await.unwrap();

    loop {
        let (conn, _addr) = listener.accept().await.unwrap();

        let ws_conn = tokio_tungstenite::accept_async(conn).await.unwrap();

        tokio::spawn(fake_gateway_no_pong_process(ws_conn));
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();

    let token = std::env::var("BOT_TOKEN")
        .map_err(|_| {
            println!("No BOT_TOKEN env var or invalid");
            std::process::exit(1);
        })
        .unwrap();

    tokio::spawn(fake_gateway_no_pong());

    let bot = Bot::new(&token).unwrap();

    bot.run().await.unwrap();
}
