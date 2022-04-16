use tokio::sync::mpsc;
mod block;
mod p2p;
mod pow;
mod protocol;
use p2p::*;
use protocol::*;
#[tokio::main]
async fn main() {
    println!("🔗Peer ID:{}", p2p::PEER_ID.clone());
    let (response_sender, mut response_receiver) =
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
        response_sender,
    };

    behaviour.floodsub.subscribe(crate::protocol::TOPIC.clone());

    let mut swarm = SwarmBuilder::new(transp, behaviour, PEER_ID.clone())
        .executor(Box::new(|fut| {
            tokio::spawn(fut);
        }))
        .build();

    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    Swarm::listen_on(
        &mut swarm,
        "/ip4/0.0.0.0/tcp/0"
            .parse()
            .expect("can not get a local socket"),
    )
    .expect("swarm can be started");

    enum EventType {
        Input(String),
        MessageEvent(protocol::MessageEvent),
    }

    let runchain = block::Chain::new();

    loop {
        let evt = {
            tokio::select! {
                line = stdin.next_line() =>
                    {
                        Some(EventType::Input(line.expect("can not get a line").expect("can not read a line from stdin")))
                    }
                response = response_receiver.recv() =>
                    {
                        Some(EventType::MessageEvent(response.expect("can not get MessageEvent")))
                    }
                _ = swarm.select_next_some() => {
                    println!("⏩Unhandled Swarm Event");
                    None
                },
            }
        };

        if let Some(event) = evt {
            match event {
                EventType::Input(s) => {}
                EventType::MessageEvent(message_event) => match message_event {
                    MessageEvent::ChainInfo(chaininfo) => {
                        if chaininfo.genesis_hash == runchain.genesis_hash()
                            && chaininfo.topic == TOPICSTRING.to_string()
                        {
                            if chaininfo.block_height > runchain.block_height() {
                                let difference = chaininfo.block_height - runchain.block_height();
                                let request_blocks = RequestNewBlocks {
                                    event_mod: EventMod::ONE(chaininfo.peer_id.clone()),
                                    num_of_blocks: difference, // 请求的块的个数
                                };
                                let json = serde_json::to_string(&request_blocks)
                                    .expect("can jsonify response");
                                swarm
                                    .behaviour_mut()
                                    .floodsub
                                    .publish(TOPIC.clone(), json.as_bytes());
                                println!("📡向{}请求块",chaininfo.peer_id);
                                
                            }
                        }
                    }

                    MessageEvent::RequestNewBlocks(request_new_blocks) => {

                    }
                    MessageEvent::ResponseBlock(response_block) => {

                    }
                },
            }
        }
    }
}
