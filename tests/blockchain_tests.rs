use rust_blockchain::blockchain::*;
use tokio::task::JoinHandle;
use tokio_postgres::*;

async fn setup() -> (Client, JoinHandle<()>) {
    let (db_client, connection) = tokio_postgres::connect(
        "host=localhost dbname=blockchain_test user=user password=pw",
        tokio_postgres::NoTls,
    )
    .await
    .unwrap();

    // The connection object performs the actual communication with the database, so spawn it off to run on its own
    let db_task = tokio::spawn(async move {
        if let Err(e) = connection.await {
            println!("DB connection error: {}", e);
        }
    });

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
            println!("Error creating blockchain table: {:?}", err)
        }

        // Clear table
        if let Err(err) = db_client
            .execute(
                "
        DELETE FROM blocks;
        ",
                &[],
            )
            .await
        {
            println!("Error clearing blocks table: {:?}", err)
        }

    (db_client, db_task)
}

#[tokio::test]
async fn test_init_chain() {
    let (mut db_client, _) = setup().await;

    let mut chain = Chain::init(&mut db_client).await.unwrap();

    // Should have been initialized with genesis block
    assert_eq!(
        chain.latest_block.hash,
        "0A31F6A1DB36EEDF9AA5C56AB90DCC76A3ABD90C77B1198336FD1AE512193F"
    );

    let new_block = chain
        .mine_block("new block".to_owned(), &mut db_client)
        .await
        .unwrap();

    let chain2 = Chain::init(&mut db_client).await.unwrap();

    // Should have been initialized with latest block
    assert_eq!(chain2.latest_block.hash, new_block.hash);
}

#[tokio::test]
async fn test_mine_blocks() {
    let (mut db_client, _) = setup().await;

    let mut chain = Chain::init(&mut db_client).await.unwrap();

    let block1 = chain.mine_block("new block 1".to_owned(), &mut db_client).await.unwrap();
    let block2 = chain.mine_block("new block 2".to_owned(), &mut db_client).await.unwrap();
    let block3 = chain.mine_block("new block 3".to_owned(), &mut db_client).await.unwrap();

    assert_eq!(&chain.latest_block.hash, &block3.hash);

    let block1 = Chain::get_block(&mut db_client,&block1.hash).await.unwrap();
    let block2 = Chain::get_block(&mut db_client,&block2.hash).await.unwrap();
    let block3 = Chain::get_block(&mut db_client,&block3.hash).await.unwrap();

    assert_eq!(block1.id, 1);
    assert_eq!(block1.data, "new block 1");
    assert_eq!(block2.id, 2);
    assert_eq!(block2.data, "new block 2");
    assert_eq!(block3.id, 3);
    assert_eq!(block3.data, "new block 3");

    assert!(matches!(Chain::check_if_block_valid(&mut db_client, &block1).await, Ok(())));
    assert!(matches!(Chain::check_if_block_valid(&mut db_client, &block2).await, Ok(())));
    assert!(matches!(Chain::check_if_block_valid(&mut db_client, &block3).await, Ok(())));
}

#[tokio::test]
async fn test_validate_invalid_block() {
    let (mut db_client, _) = setup().await;

    let mut chain = Chain::init(&mut db_client).await.unwrap();

    let block1 = chain.mine_block("new block 1".to_owned(), &mut db_client).await.unwrap();
    let block2 = chain.mine_block("new block 2".to_owned(), &mut db_client).await.unwrap();

    let invalid_block = Block {
        id: 1,
        data: "new block 1 invalid".to_owned(),
        timestamp: 12345,
        hash: block2.hash,
        nonce: 123,
        prev_hash: block1.hash.clone(),
    };

    assert!(matches!(Chain::check_if_block_valid(&mut db_client, &invalid_block).await, Err(BlockchainError::BlockInvalid(_))));
}

#[tokio::test]
async fn test_validate_chain() {
    let (mut db_client, _) = setup().await;

    let mut chain = Chain::init(&mut db_client).await.unwrap();

    assert!(matches!(chain.validate_chain(&mut db_client).await, Ok(())));

    let _ = chain.mine_block("new block 1".to_owned(), &mut db_client).await.unwrap();
    let _ = chain.mine_block("new block 2".to_owned(), &mut db_client).await.unwrap();
    let _ = chain.mine_block("new block 3".to_owned(), &mut db_client).await.unwrap();

    assert!(matches!(chain.validate_chain(&mut db_client).await, Ok(())));
}

#[tokio::test]
async fn test_validate_invalid_chain() {
    let (mut db_client, _) = setup().await;

    let mut chain = Chain::init(&mut db_client).await.unwrap();    
    assert!(matches!(chain.validate_chain(&mut db_client).await, Ok(())));

    let _ = chain.mine_block("new block 1".to_owned(), &mut db_client).await.unwrap();
    let block2 = chain.mine_block("new block 2".to_owned(), &mut db_client).await.unwrap();
    let _ = chain.mine_block("new block 3".to_owned(), &mut db_client).await.unwrap();

    assert!(matches!(chain.validate_chain(&mut db_client).await, Ok(())));

    // Invalidate block
    let _ = db_client.execute(&format!("
        UPDATE blocks
        SET data = 'invalid data'
        WHERE hash = '{}'
    ", block2.hash), &[]).await;

    assert!(matches!(chain.validate_chain(&mut db_client).await, Err(BlockchainError::ChainInvalid(_))));
}
