use rust_blockchain::{
    blockchain::{BlockchainError, Chain},
    p2p,
    types::{EventType},
};
use std::env;
use std::error::Error;
use tokio::{
    io::{self, AsyncBufReadExt},
    sync::{mpsc},
};
use tokio_postgres;
use tracing::{error, debug, info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("starting app...");

    // Get name for DB to use for this node from args passed via cmd line on startup
    let DB_NAME = env::args()
        .nth(1)
        .ok_or_else(|| "DB name not set. call 'cargo run {DB_NAME}'")?;
    // Connect to the postgres database
    let (db_client, connection) = tokio_postgres::connect(
        &format!("host=localhost dbname={} user=user password=pw", DB_NAME),
        tokio_postgres::NoTls,
    )
    .await?;

    let (main_sender, main_rcv) = mpsc::unbounded_channel::<EventType>();
    let (p2p_sender, p2p_rcv) = mpsc::unbounded_channel::<EventType>();

    let p2p_task = tokio::spawn(p2p::init_p2p(p2p_rcv, main_sender));
    let app_task = tokio::spawn(run(db_client, p2p_sender, main_rcv));

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
    mut main_rcv: mpsc::UnboundedReceiver<EventType>,
) -> Result<(), BlockchainError> {
    // We wait until the P2P service is ready
    loop {
        if let Some(event) = main_rcv.recv().await {
            if event == EventType::InitDone {
                info!("P2P init done.");
                break;
            }
            info!("Received P2P event: {:?}", event);
        } else {
            info!("Received NONE P2P event")
        }
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
    loop {
        tokio::select! {
            event = main_rcv.recv() => {
                match event {
                    Some(EventType::SendLatestBlockRequest{receiver}) => {
                        info!("Get latest block for: {:?}", receiver);
                        let block = chain.latest_block.clone();
                        let _ = p2p_sender.send(EventType::SendLatestBlock{receiver, block});
                        },
                    Some(EventType::ReceivedChain{chain: mut incoming_chain}) => {
                        info!("Received chain");
                        println!("Chain: {:?}", incoming_chain);
                        match chain.update(&mut db_client, &mut incoming_chain).await {
                            Ok(_) => info!("Successfully updated chain."),
                            Err(err) => error!("Error updating chain: {:?}", err)
                        }
                        },
                    Some(EventType::ReceivedChainRequest{receiver}) => {
                        info!("Received chain request");
                        match Chain::get_chain(&mut db_client).await {
                            Ok(chain) => {
                                info!("SEND CHAIN");
                                let _ = p2p_sender.send(EventType::SendChain{receiver, chain});
                            },
                            Err(err) => error!("{:?}", err)
                        }
                        },
                    Some(EventType::ReceivedLatestBlock{sender, block}) => {
                            info!("Got latest block: {:?}", block);
                            // Check if our chain is the longest
                            // TODO improve/extend checks
                            if &chain.latest_block.id < &block.id {
                                    let _ = p2p_sender.send(EventType::SendChainRequest{receiver: sender});
                            } else {
                                info!("We got the longest chain, not syncing");
                            }
                        },
                    Some(EventType::ReceivedNewBlock(block)) => {
                            info!("Received new block: {:?}", block);
                            // Check if our chain is the longest
                            // TODO improve/extend checks
                           match chain.add_block(&mut db_client, block).await {
                            Ok(()) => info!("Added new block"),
                            Err(err) => error!("Error adding new block: {:?}", err)
                           }
                        }
                 _ => {}
                }
            },
            user_input = stdin.next_line() => {
                let input = user_input.expect("can read line").expect("can read line");
                match input {
                    // libp2p commands
                    _ if input.starts_with("ls p") => {
                        let _ = p2p_sender.send(EventType::ListPeers);
                    }

                    // Blockchain commands
                    _ if input.starts_with("chain validate") => {
                        if let Ok(_) = chain
                            .validate_chain(&mut db_client)
                            .await
                            .map_err(|err| println!("{:?}", err))
                        {
                            println!("chain valid.")
                        }
                    }
                    _ if input.starts_with("block mine ") => {
                        let data = input.replace("block mine ", "");
                        println!("Mining...");
                        if let Ok(block) = chain
                            .mine_block(data, &mut db_client)
                            .await
                            .map_err(|err| println!("{:?}", err))
                        {
                            let _ = p2p_sender.send(EventType::SendNewBlock(block.clone()));
                            println!("added new block");
                            println!("{:#?}", block);
                        }
                    }
                    _ if input.starts_with("block get ") => {
                        let data = input.replace("block get ", "");
                        if let Ok(block) = Chain::get_block(&mut db_client, &data).await {
                            println!("{:#?}", block)
                        }
                    }
                    _ if input.starts_with("block latest") => {
                        if let Ok(block) = Chain::get_latest_block(&mut db_client)
                            .await
                            .map_err(|err| {
                                println!("Error getting latest block: {:?}", err);
                            })
                        {
                            println!("{:#?}", block)
                        }
                    }
                    _ if input.starts_with("block validate ") => {
                        let data = input.replace("block validate ", "");
                        if let Ok(block) = Chain::get_block(&mut db_client, &data).await {
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
                    _ if input.starts_with("exit") => {
                        return Ok(());
                    }
                    _ => {
                        println!("{}", BlockchainError::Error("unkown command.".to_owned()))
                    }
                };
                println!("---------------------------");
                println!("Enter command:");
            }
        }
    }
}
