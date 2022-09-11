use async_std::io;
use futures::{prelude::*, select};
use libp2p::{gossipsub, identity, swarm::{SwarmEvent, dial_opts::{DialOpts, PeerCondition}, NetworkBehaviour}, Multiaddr, PeerId, identify::{Identify, IdentifyEvent}, kad::{Kademlia, store::MemoryStore, KademliaEvent}, NetworkBehaviour};

use libp2p::gossipsub::{
    GossipsubEvent, GossipsubMessage, IdentTopic as Topic, MessageAuthenticity, MessageId,
    ValidationMode, Gossipsub,
};
use once_cell::sync::Lazy;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

static LOCAL_KEY: Lazy<identity::Keypair> = Lazy::new(identity::Keypair::generate_ed25519);
static LOCAL_PEER_ID: Lazy<PeerId> = Lazy::new(|| PeerId::from(LOCAL_KEY.public()));

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "NetworkEvent")]
struct BlockchainBehavior {
    gossipsub: Gossipsub,
    kademlia: Kademlia<MemoryStore>,
    identify: Identify
}

enum NetworkEvent {
    Gossipsub(GossipsubEvent),
    Kademlia(KademliaEvent),
    Identify(IdentifyEvent)
}

impl From<GossipsubEvent> for NetworkEvent {
    fn from(event: GossipsubEvent) -> Self {
        Self::Gossipsub(event)
    }
}

impl From<KademliaEvent> for NetworkEvent {
    fn from(event: KademliaEvent) -> Self {
        Self::Kademlia(event)
    }
}

impl From<IdentifyEvent> for NetworkEvent {
    fn from(event: IdentifyEvent) -> Self {
        Self::Identify(event)
    }
}

pub async fn init_p2p() -> Result<(), std::io::Error> {
    println!("Local PeerId: {:?}", LOCAL_PEER_ID.clone());

    // Set up encrypted TCP Transport over Mplex and Yamux protocols
    let transport = libp2p::development_transport(LOCAL_KEY.clone()).await?;

    // Create a gossipsub topic
    let topic = Topic::new("blockchain");

    // Create a swarm to manage peers and events
    let mut swarm = {
        // To content-address message, we can take the hash of message and use it as an ID.
        let message_id_fn = |message: &GossipsubMessage| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            MessageId::from(s.finish().to_string())
        };

        // Set a custom gossip
        let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
            .validation_mode(ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            .build()
            .expect("valid config");

        // Build a gossipsub network behavior
        let mut gossipsub: gossipsub::Gossipsub = gossipsub::Gossipsub::new(
            MessageAuthenticity::Signed(LOCAL_KEY.clone()),
            gossipsub_config,
        )
        .expect("correct configuration");

        gossipsub.subscribe(&topic).unwrap();

        // add an explicit peer if one was provided
        if let Some(explicit) = std::env::args().nth(2) {
            println!("connect to peer {:?}", explicit);
            match explicit.parse() {
                Ok(id) => gossipsub.add_explicit_peer(&id),
                Err(err) => println!("Failed to parse explicit peer id: {:?}", err),
            }
        }
        //gossipsub.add_explicit_peer(&("12D3KooWFcBRGArh4tWVZURPMBdkCUnPrSwzUVF2BkFFZjNUcBhE".to_owned().parse().unwrap()));
        libp2p::Swarm::new(transport, gossipsub, LOCAL_PEER_ID.clone())
    };

    swarm
        .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
        .unwrap();

    let mut stdin = io::BufReader::new(io::stdin()).lines().fuse();

    loop {
        select! {
            line = stdin.select_next_some() => {
                    let line_unwrapped = line.unwrap();
                    if line_unwrapped.starts_with("dial peer ") {
                        let string: String = line_unwrapped.replace("dial peer ", "");
                        let mut string_split = string.split(" ");
                        let peer_id: PeerId = string_split.next().unwrap().parse().expect("User to provide valid address.");
                        let dial_opts = DialOpts::peer_id(peer_id)
                            .condition(PeerCondition::Disconnected)
                            .addresses(vec![string_split.next().unwrap().parse().unwrap()])
                            .extend_addresses_through_behaviour()
                            .build();
                        match swarm.dial(dial_opts) {
                            Ok(_) => println!("Dialed {:?}", peer_id),
                            Err(e) => println!("Dial {:?} failed: {:?}", peer_id, e),
                        };
                    }
                    if line_unwrapped.starts_with("send message ") {
                        if let Err(e) = swarm
                        .behaviour_mut()
                        .publish(topic.clone(), line_unwrapped.as_bytes())
                    {
                        println!("Publish error: {:?}", e);
                    }
                }

            },
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(GossipsubEvent::Message {
                    propagation_source: peer_id,
                    message_id: id,
                    message,
                }) => println!(
                    "Got message: {} with id: {} from peer: {:?}",
                    String::from_utf8_lossy(&message.data),
                    id,
                    peer_id
                ),
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on {:?}", address);
                }
                other => {println!("other event: {:?}", other)}
            }
        }
    }
}
