// extern crate pretty_env_logger;
use core::fmt::Error;
use rust_blockchain::{Block, BlockchainError, Chain};

fn main() -> Result<(), Error> {
    pretty_env_logger::init();

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
        println!("Enter command:");
        let _ = std::io::stdin().read_line(&mut user_input).unwrap();
        user_input = user_input.replace("\r\n", "");

        match user_input {
            _ if user_input.starts_with("chain validate") => {
                if let Ok(_) = chain.validate_chain().map_err(|err| println!("{}", err)) {
                    println!("chain valid.")
                }
            },
            _ if user_input.starts_with("exit") => {
                return Ok(());
            },
            _ => {
                println!("{}", BlockchainError::Error("unkown command".to_owned()))
            }
        }
        // if user_input.starts_with("chain validate") {
        //     match chain.validate_chain() {
        //         Ok(()) => {
        //             println!("Chain successfully validated.")
        //         }
        //         Err(err) => {
        //             println!("Chain invalid. Error: {:?}", err)
        //         }
        //     }
        // }
        // if user_input.starts_with("exit") {
        //     return Ok(());
        // }
        // if user_input.starts_with("block mine ") {
        //     let data = user_input.replace("block mine ", "");
        //     let block = chain.mine_block(data);
        //     println!("added new block");
        //     println!("{:#?}", block);
        // }

        // if user_input.starts_with("block get ") {
        //     let data = user_input.replace("block get ", "");
        //     match chain.get_block(&data) {
        //         Some(block) => {
        //             println!("{:#?}", block)
        //         }
        //         None => {
        //             println!("No block with hash {:?} exists.", data)
        //         }
        //     }
        // }

        // if user_input.starts_with("block validate ") {
        //     let data = user_input.replace("block validate ", "");
        //     match chain.get_block(&data) {
        //         Some(block) => {
        //             match chain.check_if_block_valid(block) {
        //                 Ok(()) => {
        //                     println!("Valid block hash. ID of block: {}", block.id)
        //                 }
        //                 Err(err) => {
        //                     println!("{}", err);
        //                     println!("{:?}", err);
        //                 }
        //             };
        //         }
        //         None => {
        //             println!("No block with hash {:?} exists.", data)
        //         }
        //     }
        // }
    }
}
