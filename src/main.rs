use std::error::Error;
use rust_blockchain::{Block, BlockchainError, Chain};
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;
use libp2p::{identity, PeerId};

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("starting app");

    // Create network identity for node
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    info!("Local PeerId: {:?}", local_peer_id);

    let mut chain = Chain::new();

    println!("---------------------------");
    println!("Commands available:");
    println!("block mine BLOCK_DATA");
    println!("block validate BLOCK_HASH");
    println!("block get BLOCK_HASH");
    println!("chain validate");
    println!("exit");
    println!("---------------------------");

    loop {
        let mut user_input = String::new();
        println!("---------------------------");
        println!("Enter command:");
        let _ = std::io::stdin().read_line(&mut user_input).unwrap();
        user_input = user_input.replace("\r\n", "");

        match user_input {
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
    }
}
