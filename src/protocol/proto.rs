pub const DIFFICULTY_PREFIX: &[u8; 3] = &[0, 0,0];
use crate::block::Block;
use libp2p::floodsub::Topic;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
pub static TOPICSTRING: Lazy<String> = Lazy::new(|| String::from("RUNCHAINNET"));
pub static TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("RUNCHAINNET"));

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum EventMod {
    ONE((String, String)),
    // 对于request来说，第一个String是自己的peerid，第二个是对方的peerid
    // 对于Response，第一个String是空，不需要携带，因为Response的时候无需携带自己的，
    // 其他peer收到Response的时候会检查第二个String来确定是不是发给自己的Response
}

// 虽然可以记录谁是全节点，但是我不知道怎么向全节点私发，所以干脆所有消息都用广播的方式向外发送，由节点自己决定是否响应

#[derive(Debug, Serialize, Deserialize)]
// 向外广播自己的链信息,这个一定是群发的,携带了自己的peerid
pub struct ChainInfo {
    pub peer_id: String,
    pub topic: String,
    pub genesis_hash: Vec<u8>,
    pub block_height: usize,
}

// 向某个Peer请求块
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestNewBlocks {
    pub event_mod: EventMod,  // 私发的消息，One模式，One中携带了目标Peer的地址
    pub num_of_blocks: usize, // 请求的块的个数
}

// 向某个请求Block的Peer回应块.这个结构体别人会给我发，我也会给别人回应。
// 向外发时peerid填的是对方的
#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBlock {
    pub event_mod: EventMod,
    pub num_of_blocks: usize, // 块的个数
    pub blocks: Vec<Block>,   // 块
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
    RequestNewBlocks(RequestNewBlocks), // 必须携带请求来源PeerID
    ResponseBlock(ResponseBlock),       // 比如携带向谁回应请求的目标节点的的PeerID
    NewUPINFO(NewUPINFO),               // 比如说，发送内容是，明文，通过私钥加密的明文，以及公钥
                                        // 这样能够保证不会被篡改
}

// 有了请求的来源PeerID与回应目标的PeerID,main中的loop就能够完成一些关键性任务
// libp2p中的管道不能单纯地发送MessageEvent::RequestNewBlocks(requestblock, 因为这样无法携带msg.source，也就是无法把来源方通知给
// main，会导致main在收到请求 or 向外回应的时候无法将自身id与请求id相比较 or 无法在回应的结构体中加上对方id导致对方在收到我方回应时
// 无法确定是否是发个自己的。另外还有其他两种状态，发送请求，收到回应。想想它们之间的逻辑，想想它们之间其实都需要携带我方peerid和对方peerid
// 所以必须要修改管道类型。
// 所以要修改管道类型。下午来了之后先git commit一下当前的状态，然后再改。
