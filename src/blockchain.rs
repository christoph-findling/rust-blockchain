use chrono::Utc;
use log::{error, info, trace};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::sync::Mutex;
use tokio_postgres::types::Type;
use tokio_postgres::Client;

const BLOCK_DIFFICULTY: &str = "00";
const GENESIS_BLOCK_DATA: &str = "some random newspaper headline from today";
const GENESIS_BLOCK_HASH: &str = "0A31F6A1DB36EEDF9AA5C56AB90DCC76A3ABD90C77B1198336FD1AE512193F";
const GENESIS_BLOCK_TIME: i64 = 0;

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
    IoError(std::io::Error),
    DatabaseError(tokio_postgres::Error),
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
            BlockchainError::DatabaseError(ref err) => err.fmt(f),
            BlockchainError::IoError(ref err) => err.fmt(f),
        }
    }
}

impl std::error::Error for BlockchainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BlockchainError::ChainInvalid(err) => Some(err),
            BlockchainError::BlockInvalid(_) => None,
            BlockchainError::BlockNotFound(_) => None,
            BlockchainError::IoError(err) => Some(err),
            BlockchainError::DatabaseError(err) => Some(err),
            BlockchainError::Error(_) => None,
        }
    }
}

impl std::fmt::Debug for BlockchainError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        error_chain_fmt(&self, fmt)
    }
}

impl From<tokio_postgres::Error> for BlockchainError {
    fn from(err: tokio_postgres::Error) -> Self {
        Self::DatabaseError(err)
    }
}

impl From<std::io::Error> for BlockchainError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
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
    pub latest_block: Block,
}

impl Chain {
    pub async fn init(db_client: &mut Client) -> Result<Self, BlockchainError> {
        if let Err(err) = db_client
            .execute(
                "
    CREATE TABLE IF NOT EXISTS blocks (
        hash            VARCHAR PRIMARY KEY,
        id              INT8 UNIQUE NOT NULL,
        prev_hash       VARCHAR UNIQUE NOT NULL,
        timestamp       INT8 NOT NULL,
        nonce           INT8 NOT NULL,
        data            VARCHAR NOT NULL
        )
",
                &[],
            )
            .await
        {
            error!("Error creating blockchain table: {:?}", err)
        }

        let latest_block = Chain::get_latest_block(db_client).await;

        match latest_block {
            Ok(block) => Ok(Chain::build(block)),
            Err(_) => Chain::new(db_client).await,
        }
    }

    pub async fn new(db_client: &mut Client) -> Result<Self, BlockchainError> {
        let block = Block::create_genesis();

        let statement = db_client.prepare_typed(
            "INSERT INTO blocks (hash, id, prev_hash, timestamp, nonce, data) VALUES ($1, $2, $3, $4, $5, $6)",
            &[Type::VARCHAR, Type::INT8, Type::VARCHAR, Type::INT8, Type::INT8, Type::VARCHAR],
        ).await?;

        db_client
            .execute(
                &statement,
                &[
                    &block.hash,
                    &block.id,
                    &block.prev_hash,
                    &block.timestamp,
                    &block.nonce,
                    &block.data,
                ],
            )
            .await?;

        Ok(Self {
            latest_block: block,
        })
    }

    pub fn build(latest_block: Block) -> Self {
        Self {
            latest_block,
        }
    }

    pub async fn update(&mut self, db_client: &mut Client, chain: &mut Vec<Block>) -> Result<(), BlockchainError> {

        // We simply delete all rows and insert the incoming blocks for now
        db_client.execute("
        DELETE FROM blocks;
        ",
    &[]).await?;

        let statement = db_client.prepare_typed(
            "INSERT INTO blocks (hash, id, prev_hash, timestamp, nonce, data) VALUES ($1, $2, $3, $4, $5, $6)",
            &[Type::VARCHAR, Type::INT8, Type::VARCHAR, Type::INT8, Type::INT8, Type::VARCHAR],
        ).await?;

        chain.sort_by(|a, b| a.id.cmp(&b.id));

        for (index, block) in chain.iter().enumerate() {
            db_client
            .execute(
                &statement,
                &[
                    &block.hash,
                    &block.id,
                    &block.prev_hash,
                    &block.timestamp,
                    &block.nonce,
                    &block.data,
                ],
            )
            .await?;

            if index == chain.len() - 1 {
                self.latest_block = block.clone();
            }
        }

        Ok(())
    }


    pub async fn add_block(&mut self, db_client: &mut Client, block: Block) -> Result<(), BlockchainError> {

       Chain::check_if_block_valid(db_client, &block).await?;

        let statement = db_client.prepare_typed(
            "INSERT INTO blocks (hash, id, prev_hash, timestamp, nonce, data) VALUES ($1, $2, $3, $4, $5, $6)",
            &[Type::VARCHAR, Type::INT8, Type::VARCHAR, Type::INT8, Type::INT8, Type::VARCHAR],
        ).await?;

            db_client
            .execute(
                &statement,
                &[
                    &block.hash,
                    &block.id,
                    &block.prev_hash,
                    &block.timestamp,
                    &block.nonce,
                    &block.data,
                ],
            )
            .await?;

            self.latest_block = block;

        Ok(())
    }

    pub async fn get_chain(db_client: &mut Client) -> Result<Vec<Block>, BlockchainError> {
        let res = db_client
            .query(
                
                    "
    SELECT * 
    FROM blocks
    ORDER BY id ASC
    ",
                &[],
            )
            .await;

        match res {
            Ok(row_vec) => {
                return Ok(row_vec.iter().map(|row| Block {
                    hash: row.get(0),
                    id: row.get(1),
                    prev_hash: row.get(2),
                    timestamp: row.get(3),
                    nonce: row.get(4),
                    data: row.get(5),
                }).collect::<Vec<Block>>());
            },
            Err(err) => {
                error!("Error getting chain");
                return Err(BlockchainError::DatabaseError(err));
            }
        }
    }

    pub async fn get_block(db_client: &mut Client, key: &str) -> Result<Block, BlockchainError> {
        let row = db_client
            .query_one(
                &format!(
                    "
        SELECT * 
        FROM blocks
        WHERE hash = '{}'
        ",
                    key
                ),
                &[],
            )
            .await;

        match row {
            Ok(row) => Ok(Block {
                hash: row.get(0),
                id: row.get(1),
                prev_hash: row.get(2),
                timestamp: row.get(3),
                nonce: row.get(4),
                data: row.get(5),
            }),
            Err(err) => {
                error!("Block not found: {:?}", key);
                return Err(BlockchainError::DatabaseError(err));
            }
        }
    }

    pub async fn get_latest_block(db_client: &mut Client) -> Result<Block, BlockchainError> {
        let row = db_client
            .query_one(
                "
        SELECT * 
        FROM blocks
        ORDER BY timestamp DESC
        LIMIT 1
        ",
                &[],
            )
            .await?;

        Ok(Block {
            hash: row.get(0),
            id: row.get(1),
            prev_hash: row.get(2),
            timestamp: row.get(3),
            nonce: row.get(4),
            data: row.get(5),
        })
    }

    pub async fn mine_block(
        &mut self,
        data: String,
        db_client: &mut Client,
    ) -> Result<Block, BlockchainError> {
        info!("Mining block...");
        trace!("Mining block...");

        let block = Block::new(&self.latest_block, data);

        let statement = db_client.prepare_typed(
            "INSERT INTO blocks (hash, id, prev_hash, timestamp, nonce, data) VALUES ($1, $2, $3, $4, $5, $6)",
            &[Type::VARCHAR, Type::INT8, Type::VARCHAR, Type::INT8, Type::INT8, Type::VARCHAR],
        ).await?;

        db_client
            .execute(
                &statement,
                &[
                    &block.hash,
                    &block.id,
                    &block.prev_hash,
                    &block.timestamp,
                    &block.nonce,
                    &block.data,
                ],
            )
            .await?;

        //self.blocks.insert(block.hash.clone(), block);
        self.latest_block = block;
        Ok(self.latest_block.clone())
    }

    pub async fn check_if_block_valid(
        db_client: &mut Client,
        block: &Block,
    ) -> Result<(), BlockchainError> {
        if block.id == 0 && block.hash == GENESIS_BLOCK_HASH {
            return Ok(());
        }

        let prev_block = Chain::get_block(db_client, &block.prev_hash).await?;
        if prev_block.id != block.id - 1 {
            return Err(BlockchainError::BlockInvalid(block.hash.to_owned()));
        }

        let block_hash = hasher(&block.prev_hash, &block.data, block.timestamp, block.nonce);
        if block_hash != block.hash {
            return Err(BlockchainError::BlockInvalid(block.hash.to_owned()));
        }

        Ok(())
    }

    pub async fn validate_chain(&self, db_client: &mut Client) -> Result<(), BlockchainError> {
        let block_count_row = db_client
            .query_one(
                "
        SELECT 
            COUNT (*)
        FROM blocks
        ",
                &[],
            )
            .await?;

        let block_count: i64 = block_count_row.get(0);

        if block_count != self.latest_block.id + 1 {
            return Err(BlockchainError::ChainInvalid(Box::new(
                BlockchainError::Error("number of blocks != ID of latest block + 1".to_owned()),
            )));
        }

        let mut current_block_hash = self.latest_block.hash.to_owned();
        let mut blocks_validated = 0;
        loop {
            let current_block = Chain::get_block(db_client, &current_block_hash).await?;
            match Chain::check_if_block_valid(db_client, &current_block).await {
                Ok(()) => {
                    current_block_hash = current_block.prev_hash;
                }
                Err(err) => return Err(BlockchainError::ChainInvalid(Box::new(err))),
            }

            blocks_validated += 1;

            if current_block.id == 0 {
                if blocks_validated == block_count {
                    if current_block.hash == GENESIS_BLOCK_HASH {
                        return Ok(());
                    }
                    return Err(BlockchainError::ChainInvalid(Box::new(
                        BlockchainError::Error("genesis hash invalid.".to_owned()),
                    )));
                } else {
                    return Err(BlockchainError::ChainInvalid(Box::new(
                        BlockchainError::Error("invalid chain length.".to_owned()),
                    )));
                }
            }
        }
    }
}

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq)]
pub struct Block {
    pub hash: String,
    pub id: i64,
    pub prev_hash: String,
    pub timestamp: i64,
    pub nonce: i64,
    pub data: String,
}

impl Block {
    pub fn new(prev_block: &Block, data: String) -> Self {
        let timestamp = Utc::now().timestamp();
        let threads = num_cpus::get();
        println!("threads: {}", threads);
        let (hash, nonce) = find_hash(
            &prev_block.hash,
            &data,
            timestamp,
            BLOCK_DIFFICULTY,
            threads,
        );
        Self {
            hash,
            id: prev_block.id + 1,
            prev_hash: prev_block.hash.to_owned(),
            timestamp,
            nonce,
            data,
        }
    }

    pub fn create_genesis() -> Self {
        // let timestamp = Utc::now().timestamp();
        Self {
            hash: GENESIS_BLOCK_HASH.to_owned(),
            id: 0,
            prev_hash: "null".to_owned(),
            timestamp: GENESIS_BLOCK_TIME,
            nonce: 0,
            data: GENESIS_BLOCK_DATA.to_owned(),
        }
    }
}

// Takes the input and hashes it with a new nonce until a hash with the desired block difficulty is found
// Returns the hash and the nonce

// In order to circumvent the overhead that Mutex-locking causes, each thread works on blocks of
// 100 nonces at a time before checking again if a nonce has been found. Check the benchmark file
// for details on performance

pub fn find_hash(
    prev_hash: &str,
    data: &str,
    timestamp: i64,
    block_difficulty: &str,
    threads: usize,
) -> (String, i64) {
    let shared_max_nonce = Arc::new(Mutex::new(0 as i64));
    let hash = Arc::new(Mutex::new("".to_owned()));
    let final_nonce = Arc::new(Mutex::new(0 as i64));

    crossbeam::scope(|s| {
        for _ in 0..threads {
            //println!("started thread nr. {}", thread);
            let (shared_max_nonce, hash, final_nonce) = (
                Arc::clone(&shared_max_nonce),
                Arc::clone(&hash),
                Arc::clone(&final_nonce),
            );
            s.spawn(move |_| loop {
                let mut shared_max_nonce = shared_max_nonce.lock().unwrap();
                let start_nonce = shared_max_nonce.clone();
                let end_nonce = start_nonce.clone() + 100;
                *shared_max_nonce = end_nonce;
                drop(shared_max_nonce);
                for current_nonce in start_nonce..end_nonce {
                    let hash_string = hasher(prev_hash, data, timestamp, current_nonce);
                    if !hash_string.starts_with(block_difficulty) {
                        continue;
                    }
                    if *final_nonce.lock().unwrap() == 0 {
                        *hash.lock().unwrap() = hash_string;
                        *final_nonce.lock().unwrap() = current_nonce;
                    }
                    break;
                }
                if *final_nonce.lock().unwrap() != 0 {
                    break;
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

pub fn hasher(prev_hash: &str, data: &str, timestamp: i64, nonce: i64) -> String {
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

// Synchronous hashing function only used for benchmarking, see benches/benchmark.rs
pub fn find_hash_sync(
    prev_hash: &str,
    data: &str,
    timestamp: i64,
    block_difficulty: &str,
) -> (String, i64) {
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
        if !string.starts_with(block_difficulty) {
            nonce += 1;
            continue;
        }
        return (string, nonce);
    }
}
