use crate::blockchain::Block;

#[derive(Debug)]
pub enum EventType {
    Init,
    ListPeers,
    SendMessage(String),
    GetLatestBlock,
    GotLatestBlock(Block)
}