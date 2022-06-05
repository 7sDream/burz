use burz::Bot;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let token = std::env::var("BOT_TOKEN")
        .map_err(|_| {
            println!("No BOT_TOKEN env var or invalid");
            std::process::exit(1);
        })
        .unwrap();

    let bot = Bot::new(&token).unwrap();

    bot.run().await.unwrap();
}
