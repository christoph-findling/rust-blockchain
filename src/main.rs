use rust_blockchain::{p2p, blockchain::{Chain, BlockchainError}};
use tokio::{io::{self, AsyncBufReadExt}, sync::{mpsc, oneshot}};
use std::error::Error;
use tracing::{info, error, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("starting app...");

    let (p2p_init_sender, p2p_init_rcv) = oneshot::channel();
    let (p2p_sender, p2p_rcv) = mpsc::unbounded_channel::<p2p::EventType>();

    let p2p_task = tokio::spawn(p2p::init_p2p(p2p_rcv, p2p_init_sender));
    let app_task = tokio::spawn(run(p2p_sender, p2p_init_rcv));

    tokio::select! {
        res = p2p_task => info!("p2p exited {:?}", res),
        res = app_task => info!("app exited {:?}", res),
    };

    Ok(())
}

async fn run(p2p_sender: mpsc::UnboundedSender<p2p::EventType>, p2p_init_rcv: oneshot::Receiver<p2p::EventType>) -> Result<(), std::io::Error> {

    // We wait until the P2P service is ready
    if let Err(err) = p2p_init_rcv.await {
        error!("P2P init error: {:?}", err);
        // return err;
    }
    
    let mut chain = Chain::new();

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
                let _ = p2p_sender.send(p2p::EventType::SendMessage(data));
            }
            _ if user_input.starts_with("ls p") => {
                let _ = p2p_sender.send(p2p::EventType::ListPeers);
            }
            // Blockchain commands
            _ if user_input.starts_with("chain validate") => {
                if let Ok(_) = chain.validate_chain().map_err(|err| println!("{:?}", err)) {
                    println!("chain valid.")
                }
            }
            _ if user_input.starts_with("block mine ") => {
                let data = user_input.replace("block mine ", "");
                println!("Mining...");
                if let Ok(block) = chain.mine_block(data).map_err(|err| println!("{:?}", err)) {
                    println!("added new block");
                    println!("{:#?}", block);
                }
            }
            _ if user_input.starts_with("block get ") => {
                let data = user_input.replace("block get ", "");
                if let Some(block) = chain.get_block(&data).or_else(|| {
                    println!("No block with hash {} exists.", data);
                    None
                }) {
                    println!("{:#?}", block)
                }
            }
            _ if user_input.starts_with("block validate ") => {
                let data = user_input.replace("block validate ", "");
                if let Some(block) = chain.get_block(&data).or_else(|| {
                    println!("No block with hash {} exists.", data);
                    None
                }) {
                    match chain.check_if_block_valid(block) {
                        Ok(()) => {
                            println!("Valid block hash. ID of block: {}", block.id)
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
