use chrono::Utc;
use tokio::sync::mpsc;
mod block;
mod cryptography;
mod p2p;
mod pow;
mod protocol;
use crate::block::{Block, Chain};
use p2p::*;
use protocol::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::error::TryRecvError;
static FLAG: AtomicBool = AtomicBool::new(true);
use rs_merkle::{algorithms::Sha256, Hasher, MerkleTree};
#[tokio::main]
async fn main() {
    println!("🔗Peer ID:{}", p2p::PEER_ID.clone());
    let (response_sender, mut response_receiver) =
        mpsc::unbounded_channel::<protocol::MessageEvent>();

    let (new_block_sender, mut new_block_receiver) =
        mpsc::unbounded_channel::<protocol::MessageEvent>();

    let (new_transaction_sender, mut new_transaction_receiver) =
        mpsc::unbounded_channel::<protocol::MessageEvent>();

    // Keypair::<X25519Spec>通过X25519Spec来生成DH算法中要用到的密钥对
    // DH算法：https://www.liaoxuefeng.com/wiki/1252599548343744/1304227905273889
    let auth_keys = Keypair::<X25519Spec>::new()
        .into_authentic(&KEYS)
        // 好像是用自己的私钥和对方的公钥协商得到一个最终的对称密钥，用该对称密钥进行后续的加密传输
        .expect("can create auth keys");
    let transp = TokioTcpConfig::new()
        .upgrade(upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(auth_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    let mut behaviour = RunChainBehaviour {
        floodsub: Floodsub::new(PEER_ID.clone()),
        // 这里有个比较有意思的用法 Default::default()
        mdns: libp2p::mdns::Mdns::new(Default::default())
            .await
            .expect("can't create mdns"),
        response_sender_to_main: response_sender,
        new_block_sender_to_main: new_block_sender,
        new_transations_sender: new_transaction_sender,
    };

    behaviour.floodsub.subscribe(crate::protocol::TOPIC.clone());

    let mut swarm = SwarmBuilder::new(transp, behaviour, PEER_ID.clone())
        .executor(Box::new(|fut| {
            tokio::spawn(fut);
        }))
        .build();

    Swarm::listen_on(
        &mut swarm,
        "/ip4/0.0.0.0/tcp/0"
            .parse()
            .expect("can not get a local socket"),
    )
    .expect("swarm can be started");

    enum EventType {
        IsTimeToSendChainInfo,
        MessageEvent(protocol::MessageEvent),
    }

    let runchain = Arc::new(RwLock::new(block::Chain::new()));
    let runchain_arc_copy = Arc::clone(&runchain);
    let runchain_arc_copy_copy = Arc::clone(&runchain);

    // 获取准许挖矿的管道
    let (allow_pow_sender, mut allow_pow_receiver) = tokio::sync::mpsc::channel::<bool>(1);

    let mut new_up_infos = vec![];

    tokio::task::spawn_blocking(move || {
        loop {
            // 在这里组装交易
            // new_transaction_receiver 就是给它用的,反复收上链请求。截止条件是超过几秒或者是交易池满了
            let now = std::time::Instant::now();
            // while let Ok(MessageEvent::NewUPINFO(new_upinfo)) = new_transaction_receiver.try_recv()
            // {
            //     new_up_infos.push(new_upinfo);
            //     if new_up_infos.len() >= 16 {
            //         break;
            //     }
            //     let new_now = std::time::Instant::now();
            //     if new_now.saturating_duration_since(now) > std::time::Duration::from_secs(5) {
            //         break;
            //     }
            // }

            let judge_if_time_is_up = || {
                let new_now = std::time::Instant::now();
                new_now.saturating_duration_since(now) > std::time::Duration::from_secs(5)
            };

            loop {
                match new_transaction_receiver.try_recv() {
                    Ok(MessageEvent::NewUPINFO(new_upinfo)) => {
                        new_up_infos.push(new_upinfo);
                        if new_up_infos.len() >= 16 || judge_if_time_is_up() {
                            break;
                        }
                    }
                    _ => {
                        if judge_if_time_is_up() {
                            break;
                        }
                    }
                }
            }

            // 此时new_up_infos中可能已经存放了一些upinfos
            // 先挨个做验证，把非法上链信息剔除之后就开始构建默克尔树并计算默克尔根的哈希.
            // 无论是不是空的都直接构建默克尔树。万一是空的就直接挖空块。
            // 构建完之后就可以组装块让pow服务去挖了

            // 验证签名,把能成功验证签名的NewUPINFO留下，收集到verified_up_infos中。这个是要备份的。
            // 因为如果挖矿失败，verified_up_infos中的内容要被重新收回到交易池new_up_infos中
            let verified_up_infos: Vec<NewUPINFO> = new_up_infos
                .clone()
                .into_iter()
                .filter(|n| {
                    let NewUPINFO {
                        upinfo,
                        signature,
                        public_key,
                    } = n;
                    cryptography::verify(public_key, upinfo, signature)
                })
                .collect();

            new_up_infos.clear();

            // 构建默克尔树

            // 得到计算默克尔根所需的vec
            let merkel_original_vec: Vec<String> = verified_up_infos
                .clone()
                .into_iter()
                .map(|n| n.upinfo)
                .collect();

            let leaves: Vec<[u8; 32]> = merkel_original_vec
                .iter()
                .map(|x| Sha256::hash(x.as_bytes()))
                .collect();

            let merkle_tree = MerkleTree::<Sha256>::from_leaves(&leaves);
            let merkle_root = merkle_tree.root().unwrap();
            let merkle_root = std::string::String::from_utf8(merkle_root.to_vec()).unwrap();
            // 得到默克尔根

            let blocks = runchain_arc_copy.read().unwrap();
            let main_chain_last_block = blocks.last_block();
            let height = (main_chain_last_block.height + 1) as usize;
            let previous_hash = runchain_arc_copy // 这地方可能会死锁。。？
                .read()
                .unwrap()
                .calculate_hash(main_chain_last_block)
                .unwrap();
            let timestamp = format!("{}", Utc::now());

            // 打包好块，送去挖矿
            let block = Block {
                height,
                previous_hash: previous_hash.clone(),
                timestamp: timestamp.clone(),
                merkle_root: merkle_root.clone(),
                nonce: 0,
                upinfo: vec![],
            };

            let (nonce, flag) = pow::pow_v2(block, &FLAG);

            if !flag {
                // 把upinfos放回交易池new_up_infos，
                new_up_infos = verified_up_infos;
                allow_pow_receiver.blocking_recv();
            } else {
                // 走到这个分支说明挖出了新块

                // 将block添加到主链上
                let block = Block {
                    height,
                    previous_hash,
                    timestamp,
                    merkle_root,
                    nonce,
                    upinfo: merkel_original_vec,
                };
                runchain_arc_copy
                    .write()
                    .unwrap()
                    .try_add_a_block(block)
                    .unwrap();
                println!("添加块成功，向外广播一下");
            }
        }
    });

    let get_newest_chaininfo = || {
        // 这个为什么报错
        // let t=runchain_arc_copy.read().unwrap();

        // 向外广播一下chaininfo
        let last_block = runchain_arc_copy_copy.read().unwrap();
        let last_block = last_block.last_block();

        let peer_id = PEER_ID.clone().to_string();
        let genesis_hash = runchain_arc_copy_copy.read().unwrap().genesis_hash();
        let block_height = last_block.height;
        let chain_info = ChainInfo {
            peer_id,
            topic: TOPICSTRING.clone(),
            genesis_hash,
            block_height,
        };
        chain_info
    };

    loop {
        let evt = {
            tokio::select! {
                // 把这个改成timer，正常2s向外传播一次块的信息
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(2))=>
                    {
                        Some(EventType::IsTimeToSendChainInfo)
                    }

                response = response_receiver.recv() =>
                    {
                        Some(EventType::MessageEvent(response.expect("can not get MessageEvent")))
                    }
                _ = swarm.select_next_some() => {
                    // 调用发块ChainInfo的代码
                    println!("⏩Unhandled Swarm Event");
                    None
                },
            }
        };

        //                              还有几个个问题
        //  ✔️尽可能向外发ChainInfo。
        //  ✔️pow模块怎么引入，即接收钱包客户端的广播，验证签名，加入交易池。
        //  ✔️并且挖出新块之后尽可能块地向外广播新块。
        // ✔️当其他节点收到这一讯息的时候会立即停止挖矿并验证新块并将其纳入本链，并继续开始挖矿。
        //  ✔️当发现对方链比我方链长的时候，我方如何立即停止挖矿。当完成之后马上开始挖矿。
        //
        // 这些都太依赖swarm和main的loop之间的管道了

        // libp2p从外面接受事件。把事件和数据通过管道发送给main。main只是从管道recv数据。然后通过swarm发出去相应的数据。
        // 但是p2p模块写的好像有问题。接收事件不对。有一个地方注释写的 // ResponseBlock但是实际上期望接收的是requestblock
        // 搞明白那几种block是啥先。看proto.rs中的注释。

        if let Some(event) = evt {
            match event {
                EventType::IsTimeToSendChainInfo => {
                    let chain_info = get_newest_chaininfo();
                    let json = serde_json::to_string(&chain_info).expect("can jsonify chain_info");
                    swarm
                        .behaviour_mut()
                        .floodsub
                        .publish(TOPIC.clone(), json.as_bytes());
                }
                EventType::MessageEvent(message_event) => match message_event {
                    MessageEvent::ChainInfo(chaininfo) => {
                        let partner_peer_id = chaininfo.peer_id.to_string();
                        let my_pper_id = p2p::PEER_ID.to_string();

                        if chaininfo.genesis_hash == runchain.read().unwrap().genesis_hash()
                            && chaininfo.topic == TOPICSTRING.to_string()
                        {
                            if chaininfo.block_height > runchain.read().unwrap().block_height() {
                                FLAG.store(false, Ordering::Relaxed); // 立即停止计算线程

                                let difference = chaininfo.block_height
                                    - runchain.read().unwrap().block_height();
                                let request_blocks = RequestNewBlocks {
                                    event_mod: EventMod::ONE((
                                        my_pper_id.clone(),
                                        partner_peer_id.clone(),
                                    )),
                                    num_of_blocks: difference, // 请求的块的个数
                                };
                                let json = serde_json::to_string(&request_blocks)
                                    .expect("can jsonify response");
                                swarm
                                    .behaviour_mut()
                                    .floodsub
                                    .publish(TOPIC.clone(), json.as_bytes());
                                println!(
                                    "📡由于{}链较长，已经向其请求了块，等待回应中",
                                    chaininfo.peer_id
                                );
                                loop {
                                    let new_block = new_block_receiver.recv().await.unwrap();
                                    match new_block {
                                        // 解析别人对我发来的回应块
                                        MessageEvent::ResponseBlock(resp_block) => {
                                            if resp_block.event_mod == EventMod::ONE(()) {
                                                println!("已经拿到新块了!");

                                                // 插入新块
                                                let new_blocks = resp_block.blocks;
                                                for block in new_blocks.into_iter() {
                                                    runchain
                                                        .write()
                                                        .unwrap()
                                                        .try_add_a_block(block)
                                                        .unwrap()
                                                }
                                                allow_pow_sender.send(true).await.unwrap(); //将状态归位,允许挖矿
                                                break;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            } else {
                                println!(
                                    "{}的链并不比我方链长,向外广播一发我的链最新信息",
                                    chaininfo.peer_id
                                );
                                let chain_info = get_newest_chaininfo();
                                let json = serde_json::to_string(&chain_info)
                                    .expect("can jsonify chain_info");
                                swarm
                                    .behaviour_mut()
                                    .floodsub
                                    .publish(TOPIC.clone(), json.as_bytes());
                            }
                        }
                    }

                    // 这个是收到了外面某个节点的请求，该节点想要得到本节点的新块
                    // 所以本分支是向外发新块
                    MessageEvent::RequestNewBlocks(requestblock) => {
                        // 首先检查是否是向本节点请求的新块
                        // 如果是对本节点的请求，就向外发新块

                        // 解析别人对我的请求
                        if requestblock.event_mod == EventMod::ONE(()) {
                            println!("是对我请求的新块,我必须作出回应！");

                            let numboers_of_block = requestblock.num_of_blocks;
                            let read_to_send_blocks = runchain_arc_copy_copy
                                .read()
                                .unwrap()
                                .last_n_blocks(numboers_of_block);

                            let peerid = String::from("a"); // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                                                            // 我必须得有对方的peer_id啊我曹!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                            let response_block = ResponseBlock {
                                event_mod: EventMod::ONE(()),
                                num_of_blocks: numboers_of_block,
                                blocks: read_to_send_blocks,
                            };

                            let json = serde_json::to_string(&response_block)
                                .expect("can jsonify chain_info");
                            swarm
                                .behaviour_mut()
                                .floodsub
                                .publish(TOPIC.clone(), json.as_bytes());
                        }
                    }

                    _ => {
                        let chain_info = get_newest_chaininfo();
                        let json =
                            serde_json::to_string(&chain_info).expect("can jsonify chain_info");
                        swarm
                            .behaviour_mut()
                            .floodsub
                            .publish(TOPIC.clone(), json.as_bytes());
                    }
                },
            }
        }
    }
}
