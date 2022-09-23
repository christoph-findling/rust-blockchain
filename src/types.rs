use crate::blockchain::Block;

#[derive(Debug, PartialEq)]
pub enum EventType {
    InitDone,
    ListPeers,
    SendLatestBlockRequest {
        receiver: String
    },
    SendLatestBlock {
        receiver: String,
        block: Block
    },
    ReceivedLatestBlock {
        sender: String,
        block: Block
    },
    SendNewBlock(Block),
    ReceivedNewBlock(Block),
    SendChain {
        receiver: String,
        chain: Vec<Block>
    },
    SendChainRequest {
        receiver: String
    },
    ReceivedChainRequest {
        receiver: String
    },
    ReceivedChain {
        chain: Vec<Block>
    }
}
