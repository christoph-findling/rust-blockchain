use chrono::Utc;
use log::{info, trace};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

const DIFFICULTY: &str = "00";
const GENESIS_BLOCK_DATA: &str = "genesis block";
const GENESIS_BLOCK_HASH: &str = "0A31F6A1DB36EEDF9AA5C56AB90DCC76A3ABD90C77B1198336FD1AE512193F";

fn error_chain_fmt(e: &dyn std::error::Error, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

pub enum BlockchainError {
    BlockInvalid(String),
    ChainInvalid(Box<BlockchainError>),
    BlockNotFound(String),
    MiscError(Box<dyn std::error::Error>),
    Error(String),
}

impl std::fmt::Display for BlockchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BlockchainError::ChainInvalid(_) => {
                write!(f, "blockchain invalid.")
            }
            BlockchainError::BlockInvalid(hash) => {
                write!(f, "block invalid: {}", hash)
            }
            BlockchainError::BlockNotFound(hash) => {
                write!(f, "block not found: {}", hash)
            }
            BlockchainError::Error(err) => {
                write!(f, "error: {}", err)
            }
            BlockchainError::MiscError(ref err) => err.fmt(f),
        }
    }
}

impl std::error::Error for BlockchainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BlockchainError::ChainInvalid(err) => Some(err),
            BlockchainError::BlockInvalid(_) => None,
            BlockchainError::BlockNotFound(_) => None,
            BlockchainError::MiscError(_) => None,
            BlockchainError::Error(_) => None,
        }
    }
}

impl std::fmt::Debug for BlockchainError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        error_chain_fmt(&self, fmt)
    }
}

// impl From<std::fmt::Error> for BlockchainError {
//     fn from(err: std::fmt::Error) -> Self {
//         BlockchainError::MiscError(Box::new(err))
//     }
// }

// impl From<String> for BlockchainError {
//     fn from(err: String) -> Self {
//         Self::Error(err)
//     }
// }

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct Chain {
    pub blocks: HashMap<String, Block>, // BTC uses levelDB to store key value pairs, we use a HashMap
    pub latest_block: String,
}

impl Chain {
    pub fn new() -> Self {
        let genesis_block = Block::create_genesis();
        let genesis_hash = genesis_block.hash.clone();
        Self {
            blocks: HashMap::from([(genesis_block.hash.clone(), genesis_block)]),
            latest_block: genesis_hash,
        }
    }

    pub fn get_block(&self, key: &str) -> Option<&Block> {
        self.blocks.get(key)
    }

    pub fn mine_block(&mut self, data: String) -> Result<String, BlockchainError> {
        info!("Mining block...");
        trace!("Mining block...");
        let latest_block = self
            .blocks
            .get(&self.latest_block)
            .ok_or(BlockchainError::BlockNotFound(self.latest_block.to_owned()))?;
        let block = Block::new(latest_block, data);
        let hash = block.hash.clone();
        self.blocks.insert(block.hash.clone(), block);
        self.latest_block = hash.clone();
        Ok(hash)
    }

    pub fn check_if_block_valid(&self, block: &Block) -> Result<(), BlockchainError> {
        if block.hash == GENESIS_BLOCK_HASH {
            return Ok(());
        }
        if !self.blocks.contains_key(&block.prev_hash) {
            return Err(BlockchainError::BlockNotFound(block.prev_hash.to_owned()));
        }

        let prev_block = self
            .blocks
            .get(&block.prev_hash)
            .ok_or(BlockchainError::BlockNotFound(block.prev_hash.to_owned()))?;

        if prev_block.id != block.id - 1 {
            return Err(BlockchainError::BlockInvalid(block.hash.to_owned()));
        }

        let block_hash = hasher(&block.prev_hash, &block.data, block.timestamp, block.nonce);
        let get_block = self
            .blocks
            .get(&block_hash)
            .ok_or(BlockchainError::BlockNotFound(block.prev_hash.to_owned()))?;

        if block_hash != block.hash || get_block.id != block.id {
            return Err(BlockchainError::BlockInvalid(block.hash.to_owned()));
        }

        Ok(())
    }

    #[tracing::instrument(
        name = "Validating chain"
    )]
    pub fn validate_chain(&self) -> Result<(), BlockchainError> {
        let latest_block = self.get_block(&self.latest_block).ok_or(BlockchainError::ChainInvalid(Box::new(BlockchainError::BlockNotFound(self.latest_block.to_owned()))))?;
        // let latest_block_valid = self.check_if_block_valid(&latest_block);
        // if self.blocks.len() == 1 {
        //     return latest_block_valid;
        // }
        let mut current_block_hash = &latest_block.hash;
        // genesis block is always valid
        let mut blocks_validated = 1;
        loop {
            let current_block =
                self.blocks
                    .get(current_block_hash)
                    .ok_or(BlockchainError::BlockNotFound(
                        current_block_hash.to_owned(),
                    ))?;
            if current_block.id == 0 {
                if blocks_validated == self.blocks.len() {
                    return Ok(());
                } else {
                    return Err(BlockchainError::ChainInvalid(Box::new(
                        BlockchainError::Error("invalid chain length.".to_owned()),
                    )));
                }
            }

            match self.check_if_block_valid(current_block) {
                Ok(()) => {
                    current_block_hash = &current_block.prev_hash;
                }
                Err(err) => return Err(BlockchainError::ChainInvalid(Box::new(err))),
            }
            blocks_validated += 1;
        }
    }
}

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct Block {
    pub id: u128,
    pub nonce: u32,
    pub hash: String,
    pub prev_hash: String,
    pub timestamp: i64,
    pub data: String,
}

impl Block {
    pub fn new(prev_block: &Block, data: String) -> Self {
        let timestamp = Utc::now().timestamp();
        let threads = num_cpus::get();

        let (hash, nonce) = find_hash(&prev_block.hash, &data, timestamp, DIFFICULTY, threads);
        Self {
            id: prev_block.id + 1,
            data,
            hash,
            nonce,
            timestamp,
            prev_hash: prev_block.hash.to_owned(),
        }
    }

    pub fn create_genesis() -> Self {
        let timestamp = Utc::now().timestamp();
        Self {
            id: 0,
            data: GENESIS_BLOCK_DATA.to_owned(),
            hash: GENESIS_BLOCK_HASH.to_owned(),
            nonce: 0,
            timestamp,
            prev_hash: "empty hash".to_owned(),
        }
    }
}

// Takes the input and hashes it with a new nonce until a hash with the desired difficulty is found
// Returns the hash and the nonce

// In order to circumvent the overhead that Mutex-locking causes, each thread works on blocks of
// 100 nonces at a time before checking again if a nonce has been found. Check the benchmark file
// for details on performance

pub fn find_hash(
    prev_hash: &str,
    data: &str,
    timestamp: i64,
    difficulty: &str,
    threads: usize,
) -> (String, u32) {
    let shared_max_nonce = Arc::new(Mutex::new(0));
    let hash = Arc::new(Mutex::new("".to_owned()));
    let final_nonce = Arc::new(Mutex::new(0));

    crossbeam::scope(|s| {
        for _ in 0..threads {
            //println!("started thread nr. {}", thread);
            let (shared_max_nonce, hash, final_nonce) = (
                Arc::clone(&shared_max_nonce),
                Arc::clone(&hash),
                Arc::clone(&final_nonce),
            );
            s.spawn(move |_| 'looop: loop {
                let mut shared_max_nonce = shared_max_nonce.lock().unwrap();
                let start_nonce = shared_max_nonce.clone();
                let end_nonce = start_nonce.clone() + 100;
                *shared_max_nonce = end_nonce;
                drop(shared_max_nonce);
                for current_nonce in start_nonce..end_nonce {
                    let hash_string = hasher(prev_hash, data, timestamp, current_nonce);
                    if !hash_string.starts_with(difficulty) {
                        continue;
                    }
                    if *final_nonce.lock().unwrap() == 0 {
                        *hash.lock().unwrap() = hash_string;
                        *final_nonce.lock().unwrap() = current_nonce;
                    }
                    break;
                }
                if *final_nonce.lock().unwrap() != 0 {
                    break 'looop;
                }
            });
        }
    })
    .unwrap();

    (
        Arc::try_unwrap(hash).unwrap().into_inner().unwrap(),
        Arc::try_unwrap(final_nonce).unwrap().into_inner().unwrap(),
    )
}

pub fn hasher(prev_hash: &str, data: &str, timestamp: i64, nonce: u32) -> String {
    let json = serde_json::json!({
        "prev_hash": prev_hash,
        "data": data,
        "timestamp": timestamp,
        "nonce": nonce
    });
    let string = json.to_string();
    let bytes = string.as_bytes();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let byte_vec = hasher.finalize().as_slice().to_owned();
    // Convert Vec<u8> to Hex String
    let string: String = byte_vec.iter().fold("".to_owned(), |mut acc, el| {
        acc.push_str(&format!("{:X?}", el));
        acc
    });
    string
}

pub fn find_hash_sync(
    prev_hash: &str,
    data: &str,
    timestamp: i64,
    difficulty: &str,
) -> (String, u32) {
    let mut nonce = 0;
    loop {
        let json = serde_json::json!({
            "prev_hash": prev_hash,
            "data": data,
            "timestamp": timestamp,
            "nonce": nonce
        });
        let string = json.to_string();
        let bytes = string.as_bytes();
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let byte_vec = hasher.finalize().as_slice().to_owned();
        // Convert Vec<u8> to Hex String
        let string: String = byte_vec.iter().fold("".to_owned(), |mut acc, el| {
            acc.push_str(&format!("{:X?}", el));
            acc
        });
        if !string.starts_with(difficulty) {
            nonce += 1;
            continue;
        }
        return (string, nonce);
    }
}
