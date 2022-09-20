use std::env;
use rust_blockchain::{
    blockchain::{BlockchainError, Chain},
    p2p,
    types::{self, EventType}
};
use std::error::Error;
use tokio::{
    io::{self, AsyncBufReadExt},
    sync::{mpsc, oneshot},
};
use tokio_postgres;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("starting app...");

    // Get name for DB to use for this node from args passed via cmd line on startup 
    let DB_NAME = env::args().nth(1).ok_or_else(|| "DB name not set. call 'cargo run {DB_NAME}'")?;
    // Connect to the postgres database
    let (client, connection) =
        tokio_postgres::connect(&format!("host=localhost dbname={} user=user password=pw", DB_NAME), tokio_postgres::NoTls).await?;

    let (p2p_init_sender, p2p_init_rcv) = oneshot::channel();
    let (p2p_sender, p2p_rcv) = mpsc::unbounded_channel::<EventType>();

    let p2p_task = tokio::spawn(p2p::init_p2p(p2p_rcv, p2p_init_sender));
    let app_task = tokio::spawn(run(client, p2p_sender, p2p_init_rcv));

    // The connection object performs the actual communication with the database, so spawn it off to run on its own
    let db_task = tokio::spawn(async move {
        if let Err(e) = connection.await {
            error!("DB connection error: {}", e);
        }
    });

    tokio::select! {
        res = p2p_task => info!("p2p exited {:?}", res),
        res = app_task => info!("app exited {:?}", res),
        res = db_task => info!("db connection lost {:?}", res),
    };

    Ok(())
}

async fn run(
    mut db_client: tokio_postgres::Client,
    p2p_sender: mpsc::UnboundedSender<EventType>,
    p2p_init_rcv: oneshot::Receiver<EventType>,
) -> Result<(), BlockchainError> {

    // We wait until the P2P service is ready
    if let Err(err) = p2p_init_rcv.await {
        error!("P2P init error: {:?}", err);
    }

    let mut chain = Chain::init(&mut db_client).await?;

    println!("---------------------------");
    println!("Commands available:");
    println!("block mine BLOCK_DATA");
    println!("block validate BLOCK_HASH");
    println!("block get BLOCK_HASH");
    println!("chain validate");
    println!("ls p //show all peers");
    println!("exit");
    println!("---------------------------");
    println!("Enter command:");

    let mut stdin = io::BufReader::new(io::stdin()).lines();

    while let Some(user_input) = stdin.next_line().await? {
        match user_input {

            // libp2p commands
            _ if user_input.starts_with("send message ") => {
                let data = user_input.replace("send message ", "");
                let _ = p2p_sender.send(EventType::SendMessage(data));
            }
            _ if user_input.starts_with("ls p") => {
                let _ = p2p_sender.send(EventType::ListPeers);
            }

            // Blockchain commands
            _ if user_input.starts_with("chain validate") => {
                if let Ok(_) = chain
                    .validate_chain(&mut db_client)
                    .await
                    .map_err(|err| println!("{:?}", err))
                {
                    println!("chain valid.")
                }
            }
            _ if user_input.starts_with("block mine ") => {
                let data = user_input.replace("block mine ", "");
                println!("Mining...");
                if let Ok(block) = chain
                    .mine_block(data, &mut db_client)
                    .await
                    .map_err(|err| println!("{:?}", err))
                {
                    println!("added new block");
                    println!("{:#?}", block);
                }
            }
            _ if user_input.starts_with("block get ") => {
                let data = user_input.replace("block get ", "");
                if let Ok(block) = Chain::get_block( &mut db_client, &data).await {
                    println!("{:#?}", block)
                }
            }
            _ if user_input.starts_with("block latest") => {
                if let Ok(block) = Chain::get_latest_block(&mut db_client).await.map_err(|err| {
                    println!("Error getting latest block: {:?}", err);
                }) {
                    println!("{:#?}", block)
                }
            }
            _ if user_input.starts_with("block validate ") => {
                let data = user_input.replace("block validate ", "");
                if let Ok(block) = Chain::get_block( &mut db_client, &data).await {
                    match Chain::check_if_block_valid(&mut db_client, &block).await {
                        Ok(()) => {
                            println!("Valid block. ID of block: {}", block.id)
                        }
                        Err(err) => {
                            println!("{:?}", err);
                        }
                    };
                }
            }
            _ if user_input.starts_with("exit") => {
                return Ok(());
            }
            _ => {
                println!("{}", BlockchainError::Error("unkown command.".to_owned()))
            }
        };

        println!("---------------------------");
        println!("Enter command:");
    }

    Ok(())
}
