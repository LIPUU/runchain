pub const DIFFICULTY_PREFIX: &[u8; 4] = &[0, 0, 0, 0];
use crate::block::Block;
use libp2p::floodsub::Topic;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
pub static TOPICSTRING: Lazy<String> = Lazy::new(|| String::from("RUNCHAINNET"));
pub static TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("RUNCHAINNET"));

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum EventMod {
    ALL,
    ONE(String),
}

// 虽然可以记录谁是全节点，但是我不知道怎么向全节点私发，所以干脆所有消息都用广播的方式向外发送，由节点自己决定是否响应

#[derive(Debug, Serialize, Deserialize)]
// 向外广播自己的链信息,这个一定是群发的
pub struct ChainInfo {
    pub peer_id: String,
    pub topic: String,
    pub genesis_hash: String,
    pub block_height: usize,
}

// 向某个Peer请求块
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestNewBlocks {
    pub event_mod: EventMod,  // 私发的消息，One模式，One中携带了目标Peer的地址
    pub num_of_blocks: usize, // 请求的块的个数
}

// 向某个请求Block的Peer回应块.这个结构体别人会给我发，我也会给别人回应。
#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBlock {
    pub event_mod: EventMod,
    pub num_of_blocks: u8,  // 块的个数
    pub blocks: Vec<Block>, // 块
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewUPINFO {
    pub upinfo: String,
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MessageEvent {
    ChainInfo(ChainInfo),
    RequestNewBlocks(RequestNewBlocks),
    ResponseBlock(ResponseBlock),
    NewUPINFO(NewUPINFO), // 比如说，发送内容是，明文，通过私钥加密的明文的Hash，以及公钥
                          // 这样能够保证不会被篡改
}
