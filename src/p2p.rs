// p2p.rs负责解析与其他节点的信息
// 然后把信息通过管道反馈给miner_node.rs以及light_node.rs
// miner_node.rs负责总调度
// 服务类型：
//      高度检测。用于检查链长度。如果长度小于其他链长度，就通过队列反馈给miner。
//      请求块服务。请求其他节点把块打包好送回来，然后miner将它上链
//      交易服务。  负责监听light_node发来的交易请求。把交易请求发送给miner，miner负责验证后放入交易池(vec)
pub use libp2p::{
    core::upgrade,
    floodsub::{Floodsub, FloodsubEvent, Topic},
    futures::StreamExt,
    identity,
    mdns::{Mdns, MdnsEvent},
    mplex,
    noise::{Keypair, NoiseConfig, X25519Spec},
    swarm::{NetworkBehaviourEventProcess, SwarmBuilder},
    tcp::TokioTcpConfig,
    NetworkBehaviour, PeerId, Swarm, Transport,
};
pub use once_cell::sync::Lazy;
pub use serde::{Deserialize, Serialize};
pub use std::collections::HashSet;
pub use tokio::{fs, io::AsyncBufReadExt, sync::mpsc};

pub static KEYS: Lazy<identity::Keypair> = Lazy::new(|| identity::Keypair::generate_ed25519());
pub static PEER_ID: Lazy<PeerId> = Lazy::new(|| PeerId::from(KEYS.public()));
use crate::protocol::{ChainInfo, MessageEvent, ResponseBlock};

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
pub struct RunChainBehaviour {
    pub floodsub: Floodsub,
    pub mdns: Mdns,
    #[behaviour(ignore)]
    pub response_sender_to_main: mpsc::UnboundedSender<MessageEvent>,
    #[behaviour(ignore)]
    pub new_block_sender_to_main: mpsc::UnboundedSender<(MessageEvent, String)>,
    #[behaviour(ignore)]
    pub new_transations_sender: mpsc::UnboundedSender<(MessageEvent, String)>,
}

impl NetworkBehaviourEventProcess<FloodsubEvent> for RunChainBehaviour {
    fn inject_event(&mut self, event: FloodsubEvent) {
        match event {
            FloodsubEvent::Message(msg) => {
                match serde_json::from_slice::<MessageEvent>(&msg.data) {
                    Ok(MessageEvent::ChainInfo(chaininfo)) => {
                        println!("💎收到了节点{}的ChainInfo广播", msg.source);
                        self.report_to_loop_got_info_or_request(MessageEvent::ChainInfo(chaininfo));
                        return;
                    }
                    Ok(MessageEvent::RequestNewBlocks(requestblock)) => {
                        println!("😆{}节点要求请求新块!", msg.source);
                        self.report_to_loop_got_info_or_request(MessageEvent::RequestNewBlocks(
                            requestblock,
                        ));
                        return;
                    }

                    // ResponseBlock
                    Ok(MessageEvent::ResponseBlock(response_block)) => {
                        println!("😆收到了{}节点发来的新块!", msg.source);
                        self.report_to_loop_got_new_block(
                            MessageEvent::ResponseBlock(response_block),
                            msg.source.to_string(),
                        );
                        return;
                    }

                    // 在这里处理新收到的交易请求
                    Ok(MessageEvent::NewUPINFO(newupinfo)) => {
                        println!("😆钱包节点{}发来上链请求!", msg.source);
                        self.report_to_loop_got_new_upinfo(
                            MessageEvent::NewUPINFO(newupinfo),
                            msg.source.to_string(),
                        );
                        return;
                    }

                    _ => {
                        println!("Unexpected message");
                        return;
                    }
                };
            }
            _ => (),
        }
    }
}

impl RunChainBehaviour {
    fn report_to_loop_got_info_or_request(&self, message_event: MessageEvent) {
        self.response_sender_to_main.send(message_event).unwrap();
    }
    fn report_to_loop_got_new_block(&self, new_block: MessageEvent, source_peer_id: String) {
        self.new_block_sender_to_main
            .send((new_block, source_peer_id))
            .unwrap();
    }

    fn report_to_loop_got_new_upinfo(&self, new_block: MessageEvent, source_peer_id: String) {
        self.new_transations_sender
            .send((new_block, source_peer_id))
            .unwrap();
    }
}
// 这个是mdns提供的事件，可以是节点发现事件，也可以是节点过期事件
impl NetworkBehaviourEventProcess<MdnsEvent> for RunChainBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(discovered_list) => {
                println!("🌟🌟MdnsEvent::Discovered->发现新节点!");
                for (peer, _addr) in discovered_list {
                    println!("🌟{}", peer);
                    // 把发现的节点给添加到可以发消息的peer队列中
                    self.floodsub.add_node_to_partial_view(peer);
                }
            }
            MdnsEvent::Expired(expired_list) => {
                println!("✨MdnsEvent::Expired->有节点过期了!");
                for (peer, _addr) in expired_list {
                    if !self.mdns.has_node(&peer) {
                        self.floodsub.remove_node_from_partial_view(&peer);
                    }
                }
            }
        }
    }
}
