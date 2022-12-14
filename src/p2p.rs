use futures::{prelude::*, select};
use libp2p::{
    core::transport::upgrade,
    gossipsub::{
        Gossipsub, GossipsubConfigBuilder, GossipsubEvent, GossipsubMessage, IdentTopic as Topic,
        MessageAuthenticity, MessageId, ValidationMode,
    },
    identity,
    mdns::{MdnsEvent, TokioMdns},
    mplex, noise,
    swarm::{
        dial_opts::{DialOpts, PeerCondition},
        SwarmBuilder, SwarmEvent,
    },
    tcp::{GenTcpConfig, TokioTcpTransport},
    Multiaddr, NetworkBehaviour, PeerId, Swarm, Transport,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use tokio::{
    io::{self, AsyncBufReadExt},
    sync::{mpsc},
};
use tracing::{debug, error, Level};

use crate::blockchain::Block;
use crate::types::EventType;

// Generate local keypair
static LOCAL_KEY: Lazy<identity::Keypair> = Lazy::new(identity::Keypair::generate_ed25519);
static LOCAL_PEER_ID: Lazy<PeerId> = Lazy::new(|| PeerId::from(LOCAL_KEY.public()));
// Create a gossipsub topic
static TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("blockchain"));

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceivedLatestBlock {
    receiver: String,
    block: Block,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceivedNewBlock {
    block: Block,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceivedChain {
    receiver: String,
    chain: Vec<Block>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChainRequest {
    receiver: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LatestBlockRequest {
    receiver: String,
    random: bool,
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "NetworkEvent")]
struct BlockchainBehavior {
    gossipsub: Gossipsub,
    mdns: TokioMdns,
}

enum NetworkEvent {
    Gossipsub(GossipsubEvent),
    TokioMdns(MdnsEvent),
}

impl From<GossipsubEvent> for NetworkEvent {
    fn from(event: GossipsubEvent) -> Self {
        Self::Gossipsub(event)
    }
}

impl From<MdnsEvent> for NetworkEvent {
    fn from(event: MdnsEvent) -> Self {
        Self::TokioMdns(event)
    }
}

pub async fn init_p2p(
    mut rx_rcv: mpsc::UnboundedReceiver<EventType>,
    main_sender: mpsc::UnboundedSender<EventType>,
) -> Result<(), std::io::Error> {
    println!("Local PeerId: {:?}", LOCAL_PEER_ID.clone());

    // We manually keep track of all currently connected gossipsub peers
    // in order to keep the borrow checker happy (otherwise we would need
    // acces to both the gossipsub and mdns behaviours at the same time)
    let mut gossipsub_peers: HashSet<PeerId> = HashSet::<PeerId>::new();

    // Create a keypair for authenticated encryption of the transport.
    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&LOCAL_KEY)
        .expect("Signing libp2p-noise static DH keypair failed.");

    // Create a tokio-based TCP transport use noise for authenticated
    // encryption and Mplex for multiplexing of substreams on a TCP stream.
    let transport = TokioTcpTransport::new(GenTcpConfig::default().nodelay(true))
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    // Create a swarm to manage peers and events
    let mut swarm = {
        let mut blockchain_behavior = BlockchainBehavior {
            gossipsub: build_gossipsub_behavior(),
            mdns: TokioMdns::new(Default::default())
                .await
                .expect("can create mdns"),
        };

        SwarmBuilder::new(transport, blockchain_behavior, LOCAL_PEER_ID.clone())
            // We want the connection background tasks to be spawned
            // onto the tokio runtime.
            .executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
            .build()
    };

    swarm
        .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
        .unwrap();

    if let Err(err) = main_sender.send(EventType::InitDone) {
        println!("P2P init sending error: {:?}", err);
    }

    loop {
        tokio::select! {
            event = rx_rcv.recv() => {
                match event {
                    Some(EventType::ListPeers) => {
                        println!("discovered nodes (mdns): {:?}", swarm
                        .behaviour_mut()
                        .mdns
                        .discovered_nodes().collect::<Vec<_>>());

                        println!("connected peers (gossipsub): {:?}", swarm
                        .behaviour_mut()
                        .gossipsub
                        .all_peers().collect::<Vec<_>>());
                    },
                    Some(EventType::SendLatestBlock{block, receiver}) => {
                        debug!("Send latest block to {:?}", receiver);
                        let req = ReceivedLatestBlock{receiver, block};
                        let json = serde_json::to_string(&req).expect("can jsonify request");

                        if let Err(e) = swarm
                        .behaviour_mut()
                        .gossipsub
                        .publish(TOPIC.clone(), json.as_bytes())
                        {
                            println!("Publish error: {:?}", e);
                        }
                    },
                    Some(EventType::SendNewBlock(block)) => {
                        debug!("Broadcast new block");
                        let req = ReceivedNewBlock{block};
                        let json = serde_json::to_string(&req).expect("can jsonify request");

                        if let Err(e) = swarm
                        .behaviour_mut()
                        .gossipsub
                        .publish(TOPIC.clone(), json.as_bytes())
                        {
                            println!("Publish error: {:?}", e);
                        }
                    },
                    Some(EventType::SendChainRequest{receiver}) => {
                        debug!("Send chain request to {:?}", receiver);
                        let req = ChainRequest{receiver};
                        let json = serde_json::to_string(&req).expect("can jsonify request");

                        if let Err(e) = swarm
                        .behaviour_mut()
                        .gossipsub
                        .publish(TOPIC.clone(), json.as_bytes())
                        {
                            println!("Publish error: {:?}", e);
                        }
                    },
                    Some(EventType::SendChain{receiver, chain}) => {
                        debug!("Send chain to {:?}", receiver);
                        let req = ReceivedChain{receiver, chain};
                        let json = serde_json::to_string(&req).expect("can jsonify request");

                        if let Err(e) = swarm
                        .behaviour_mut()
                        .gossipsub
                        .publish(TOPIC.clone(), json.as_bytes())
                        {
                            println!("Publish error: {:?}", e);
                        }
                    },
                    None => {
                        debug!("p2p channel closed.");
                        return Ok(());
                    },
                    _ => {

                    },
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(NetworkEvent::Gossipsub(event)) =>
                    {
                        match event {
                            GossipsubEvent::Subscribed{peer_id, topic} => {
                                debug!("Gossipsub Subscribed | PeerId: {:?}, Topic: {:?}", peer_id, topic);
                                if gossipsub_peers.len() == 0 {
                                    gossipsub_peers.insert(peer_id);
                                    // Request latest block from peer on first connect/reconnect
                                    let req = LatestBlockRequest{receiver: peer_id.to_string(), random: true};
                                    let json = serde_json::to_string(&req).expect("can jsonify request");

                                    if let Err(e) = swarm
                                    .behaviour_mut()
                                    .gossipsub
                                    .publish(TOPIC.clone(), json.as_bytes())
                                    {
                                        println!("Publish error: {:?}", e);
                                    }
                                    continue;
                                }
                                gossipsub_peers.insert(peer_id);
                            },
                            GossipsubEvent::Message{propagation_source, message_id: _, message} => {
                                if let Ok(resp) = serde_json::from_slice::<ReceivedLatestBlock>(&message.data) {
                                    if resp.receiver == LOCAL_PEER_ID.to_string() {
                                        debug!("ReceivedLatestBlock from {:?}:", message.source);
                                        if let Some(source) = message.source {
                                            if let Err(err) = main_sender.send(EventType::ReceivedLatestBlock{sender: source.to_string(), block: resp.block}) {
                                                debug!("P2P to main ReceivedLatestBlock error: {:?}", err);
                                            }
                                        } else {
                                                debug!("no message source")
                                            }
                                    }
                                } else if let Ok(req) = serde_json::from_slice::<LatestBlockRequest>(&message.data) {
                                    if req.receiver == LOCAL_PEER_ID.to_string() {
                                        debug!("SendLatestBlockRequest from {:?}:", message.source);
                                        if let Some(source) = message.source {
                                        if let Err(err) = main_sender.send(EventType::SendLatestBlockRequest{receiver: source.to_string()}) {
                                            debug!("P2P to main SendLatestBlockRequest error: {:?}", err);
                                        }
                                    } else {
                                            debug!("no message source")
                                        }
                                    }
                                } else if let Ok(req) = serde_json::from_slice::<ChainRequest>(&message.data) {
                                    if req.receiver == LOCAL_PEER_ID.to_string() {
                                        debug!("ChainRequest from {:?}:", message.source);
                                        if let Some(source) = message.source {
                                            if let Err(err) = main_sender.send(EventType::ReceivedChainRequest{receiver: source.to_string()}) {
                                                debug!("P2P to main ReceivedChainRequest error: {:?}", err);
                                            }
                                        } else {
                                            debug!("no message source")
                                        }
                                    }
                                } else if let Ok(res) = serde_json::from_slice::<ReceivedChain>(&message.data) {
                                    if res.receiver == LOCAL_PEER_ID.to_string() {
                                        debug!("ReceivedChain from {:?}:", message.source);
                                        if let Err(err) = main_sender.send(EventType::ReceivedChain{chain: res.chain}) {
                                            debug!("P2P to main ReceivedChainRequest error: {:?}", err);
                                        }
                                    }
                                } else if let Ok(res) = serde_json::from_slice::<ReceivedNewBlock>(&message.data) {
                                    if propagation_source != LOCAL_PEER_ID.clone() {
                                        debug!("ReceivedNewBlock from {:?}:", message.source);
                                        if let Err(err) = main_sender.send(EventType::ReceivedNewBlock(res.block)) {
                                            debug!("P2P to main ReceivedNewBlock error: {:?}", err);
                                        }
                                    }
                                }
                                //debug!("Gossipsub Message | PropagationSource: {:?}, MesssageId: {:?}, Message: {:?}", propagation_source, message_id, message);
                            },
                            GossipsubEvent::Unsubscribed{peer_id, topic} => {
                                gossipsub_peers.remove(&peer_id);
                                debug!("Gossipsub Unsubscribed | PeerId: {:?}, Topic: {:?}", peer_id, topic);
                            },
                            GossipsubEvent::GossipsubNotSupported{peer_id} => {
                                gossipsub_peers.remove(&peer_id);
                                debug!("Gossipsub NotSupported | PeerId: {:?}", peer_id);
                            },
                        }
                    },
                SwarmEvent::Behaviour(NetworkEvent::TokioMdns(event)) =>
                    match event {
                        // On each Discovered event, we connect to all newly discovered peers
                        MdnsEvent::Discovered(peers) => {
                            println!("discovered event called");
                            let mut unique_peers = HashMap::<PeerId, Multiaddr>::new();
                            for (peer, addr) in peers {
                                debug!("discovered peer {} {}", peer, addr);
                                    unique_peers.entry(peer).or_insert(addr);
                            }
                            let unique_vec = unique_peers.iter().collect::<Vec<_>>();
                            for (peer, addr) in unique_vec {
                                // Check if not already connected to Peer
                                if !gossipsub_peers.contains(peer) {
                                    dial_peer(&mut swarm, peer, addr);
                                }
                            }
                        },
                        MdnsEvent::Expired(expired) => {
                            for (peer, addr) in expired {
                                debug!("peer expired {} {}", peer, addr);
                            }
                        },
                    },
                SwarmEvent::IncomingConnection { local_addr, .. } =>
                debug!("SwarmEvent IncomingConnection Address: {:?}", local_addr),
                SwarmEvent::IncomingConnectionError { local_addr, send_back_addr: _, error } =>
                debug!("SwarmEvent IncomingConnectionError Address: {:?} Error: {:?}", local_addr, error),
                SwarmEvent::NewListenAddr { address, .. } =>
                debug!("SwarmEvent NewListenAddr Address: {:?}", address),
                SwarmEvent::ConnectionClosed{ peer_id, endpoint: _, num_established: _, cause } => {
                    // As soon as a Peer disconnects, we have to manually remove him from our gossipsub peers
                    debug!("SwarmEvent ConnectionClosed PeerId: {:?} | Cause: {:?}", peer_id, cause);
                    swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                    gossipsub_peers.remove(&peer_id);
                },
                SwarmEvent::ConnectionEstablished{peer_id, ..} => {
                    debug!("SwarmEvent ConnectionEstablished PeerId: {:?}", peer_id);
                },
                SwarmEvent::OutgoingConnectionError{peer_id, ..} => {
                    debug!("SwarmEvent OutgoingConnectionError PeerId: {:?}", peer_id);
                },
                SwarmEvent::ExpiredListenAddr{listener_id, ..} => {
                    debug!("SwarmEvent ExpiredListenAddr ListenerId: {:?}", listener_id);
                },
                SwarmEvent::ListenerClosed{listener_id, ..} => {
                    debug!("SwarmEvent ListenerClosed ListenerId: {:?}", listener_id);
                },
                SwarmEvent::ListenerError{listener_id, ..} => {
                    debug!("SwarmEvent ListenerError ListenerId: {:?}", listener_id);
                },
                _ =>
                debug!("got other swarm event")

            }
        }
    }
}

fn dial_peer(swarm: &mut Swarm<BlockchainBehavior>, peer_id: &PeerId, addr: &Multiaddr) {
    let dial_opts = DialOpts::peer_id(peer_id.clone())
        // NotDialing == not dialing + not connected
        .condition(PeerCondition::NotDialing)
        .addresses(vec![addr.clone()])
        // If we extend the address and connect to the same peer on multiple addresses,
        // a connection error is thrown for the second and all following connections.
        // .extend_addresses_through_behaviour()
        .build();
    match &swarm.dial(dial_opts) {
        Ok(_) => println!("Dialed {:?} {:?}", peer_id, addr),
        Err(e) => println!("Dial {:?} failed: {:?}", peer_id, e),
    };
}

fn build_gossipsub_behavior() -> Gossipsub {
    // To content-address message, we can take the hash of message and use it as an ID.
    let message_id_fn = |message: &GossipsubMessage| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        MessageId::from(s.finish().to_string())
    };

    // Set a custom gossip
    let gossipsub_config = GossipsubConfigBuilder::default()
        .validation_mode(ValidationMode::Strict)
        .message_id_fn(message_id_fn)
        .build()
        .expect("valid config");

    // Build a gossipsub network behavior
    let mut gossipsub: Gossipsub = Gossipsub::new(
        MessageAuthenticity::Signed(LOCAL_KEY.clone()),
        gossipsub_config,
    )
    .expect("correct configuration");

    gossipsub.subscribe(&TOPIC).unwrap();

    gossipsub
}
