use rust_blockchain::{blockchain::*};

#[test]
fn test_create_chain() {
    let chain = Chain::new();

    assert_eq!(chain.blocks.len(), 1);

    let genesis_block = chain.blocks.get(&chain.latest_block).unwrap();
    assert_eq!(genesis_block.id, 0);
    assert_eq!(genesis_block.data, "genesis block");
    assert_eq!(genesis_block.hash.starts_with("0"), true);
}

#[test]
fn test_add_blocks() {
    let mut chain = Chain::new();
    let block1_hash = chain.mine_block("new block 1".to_owned()).unwrap();
    let block2_hash = chain.mine_block("new block 2".to_owned()).unwrap();
    let block3_hash = chain.mine_block("new block 3".to_owned()).unwrap();

    assert_eq!(chain.blocks.len(), 4);

    let block1 = chain.blocks.get(&block1_hash).unwrap();
    let block2 = chain.blocks.get(&block2_hash).unwrap();
    let block3 = chain.blocks.get(&block3_hash).unwrap();

    assert!(matches!(chain.check_if_block_valid(block1), Ok(())));
    assert!(matches!(chain.check_if_block_valid(block2), Ok(())));
    assert!(matches!(chain.check_if_block_valid(block3), Ok(())));

    assert_eq!(block1.id, 1);
    assert_eq!(block1.data, "new block 1");
    assert_eq!(block2.id, 2);
    assert_eq!(block2.data, "new block 2");
    assert_eq!(block3.id, 3);
    assert_eq!(block3.data, "new block 3");
}

#[test]
fn test_validate_invalid_block() {
    let mut chain = Chain::new();

    let block1_hash = chain.mine_block("new block 1".to_owned()).unwrap();
    let block2_hash = chain.mine_block("new block 2".to_owned()).unwrap();

    let invalid_block = Block {
        id: 1,
        data: "new block 1 invalid".to_owned(),
        timestamp: 12345,
        hash: block2_hash,
        nonce: 123,
        prev_hash: block1_hash.clone(),
    };

    assert!(matches!(chain.check_if_block_valid(&invalid_block), Err(_)));
}

#[test]
fn test_validate_chain() {
    let mut chain = Chain::new();
    assert!(matches!(chain.validate_chain(), Ok(())));

    let _ = chain.mine_block("new block 1".to_owned()).unwrap();
    let _ = chain.mine_block("new block 2".to_owned()).unwrap();
    let _ = chain.mine_block("new block 3".to_owned()).unwrap();

    assert!(matches!(chain.validate_chain(), Ok(())));
}

#[test]
fn test_validate_invalid_chain() {
    let mut chain = Chain::new();
    assert!(matches!(chain.validate_chain(), Ok(())));

    let _ = chain.mine_block("new block 1".to_owned()).unwrap();
    let block2_hash = chain.mine_block("new block 2".to_owned()).unwrap();
    let _ = chain.mine_block("new block 3".to_owned()).unwrap();

    assert!(matches!(chain.validate_chain(), Ok(())));

    let block2 = chain.blocks.get_mut(&block2_hash).unwrap();
    block2.data = "invalid block".to_owned();

    assert!(matches!(chain.validate_chain(), Err(_)));
}
