use crate::commands::{
    handle_add_book, handle_list_books, handle_list_peers, handle_share_book,
    respond_with_public_books,
};
use libp2p::{
    core::upgrade,
    floodsub::{Floodsub, FloodsubEvent, Topic},
    identity,
    mdns::{Mdns, MdnsEvent},
    mplex,
    noise::{Keypair, NoiseConfig, X25519Spec},
    futures::StreamExt,
    swarm::{NetworkBehaviourEventProcess, Swarm, SwarmBuilder},
    tcp::TokioTcpConfig,
    NetworkBehaviour, PeerId, Transport,
};
use log::{error, info};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, io::AsyncBufReadExt};
mod commands;

const STORAGE_PATH: &str = "./library.json";
type Library = Vec<Book>;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

// lazy static constants
static KEYS: Lazy<identity::Keypair> = Lazy::new(|| identity::Keypair::generate_ed25519());
static PEER_ID: Lazy<PeerId> = Lazy::new(|| PeerId::from(KEYS.public()));
static TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("library"));

#[derive(Debug, Serialize, Deserialize)]
pub struct Book {
    id: usize,
    title: String,
    author: String,
    publisher: String,
    public: bool,
}

#[derive(Debug, Serialize, Deserialize)]
enum ListMode {
    ALL,
    One(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct ListRequest {
    mode: ListMode,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListResponse {
    mode: ListMode,
    data: Library,
    receiver: String,
}

enum EventType {
    Response(ListResponse),
    Input(String),
}

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
pub struct BookBehavior {
    floodsub: Floodsub,
    mdns: Mdns,
    #[behaviour(ignore)]
    response_sender: mpsc::UnboundedSender<ListResponse>,
}

impl NetworkBehaviourEventProcess<MdnsEvent> for BookBehavior {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(discovered_list) => {
                for (peer, _addr) in discovered_list {
                    self.floodsub.add_node_to_partial_view(peer);
                }
            }
            MdnsEvent::Expired(expired_list) => {
                for (peer, _addr) in expired_list {
                    if !self.mdns.has_node(&peer) {
                        self.floodsub.remove_node_from_partial_view(&peer);
                    }
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<FloodsubEvent> for BookBehavior {
    fn inject_event(&mut self, event: FloodsubEvent) {
        match event {
            FloodsubEvent::Message(msg) => {
                if let Ok(res) = serde_json::from_slice::<ListResponse>(&msg.data) {
                    if res.receiver == PEER_ID.to_string() {
                        info!("response from {}:", msg.source);
                        res.data.iter().for_each(|r| info!("{:?}", r));
                    } 
                } else if let Ok(req) = serde_json::from_slice::<ListRequest>(&msg.data) {
                    match req.mode {
                        ListMode::ALL => {
                            info!("request for all: {:?} from {:?}", req, msg.source);
                            respond_with_public_books(
                                self.response_sender.clone(),
                                msg.source.to_string(),
                            );
                        }
                        ListMode::One(ref peer_id) => {
                            if peer_id == &PEER_ID.to_string() {
                                info!("request for one: {:?} from {:?}", req, msg.source);
                                respond_with_public_books(
                                    self.response_sender.clone(),
                                    msg.source.to_string(),
                                );
                            }
                        }
                    }
                }
            }
            _ => (),
        }
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    info!("Peer Id: {}", PEER_ID.clone());

    // multi-producer, single-consumer queue for sending values across asynchronous tasks.
    // aka - async channel for communicating between different parts of the application
    let (response_sender, mut response_receiver) = mpsc::unbounded_channel();

    // authentication keys using noise protocol
    let auth_keys = Keypair::<X25519Spec>::new()
        .into_authentic(&KEYS)
        .expect("unable to create auth keys");

    // create transport
    let transport = TokioTcpConfig::new() // use Tokio's async TCP
        .upgrade(upgrade::Version::V1) //upgrade connection to use Noise protocol for secure communication
        .authenticate(NoiseConfig::xx(auth_keys).into_authenticated()) // authenticate after upgrade - NoiseConfig::xx is guaranteed to be interoperable with other libp2p apps
        .multiplex(mplex::MplexConfig::new()) // negotiate a (sub)stream multiplexer on top of authenticated transport for multiple substreams on same transport
        .boxed(); // only capture Output and Error types

    // define logic for network and peers
    // floodsub to handle events
    // mdns for discovering local peers
    let mut behavior = BookBehavior {
        floodsub: Floodsub::new(PEER_ID.clone()),
        mdns: Mdns::new(Default::default())
            .await
            .expect("unable to create mdns"),
        response_sender,
    };

    behavior.floodsub.subscribe(TOPIC.clone());

    // manage connections based on transport and behavior using tokio runtime
    let mut swarm = SwarmBuilder::new(transport, behavior, PEER_ID.clone())
        .executor(Box::new(|future| {
            tokio::spawn(future);
        }))
        .build();

    // async read stdin
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();

    // start swarm
    Swarm::listen_on(
        &mut swarm,
        "/ip4/0.0.0.0/tcp/0"
            .parse()
            .expect("unable to get local socket"),
    )
    .expect("swarm unable to start");

    // event loop
    loop {
        let event_type = {
            tokio::select! {
                line = stdin.next_line() => Some(EventType::Input(line.expect("unable to get line").expect("unable to read line from stdin"))),
                response = response_receiver.recv() => Some(EventType::Response(response.expect("unable to get response"))),
                event = swarm.select_next_some() => {
                    info!("Unhandled swarm event: {:?}", event);
                    None
                },
            }
        };

        if let Some(event) = event_type {
            match event {
                EventType::Response(res) => {
                    let json =
                        serde_json::to_string(&res).expect("unable to jsonify event type response");
                    swarm.behaviour_mut().floodsub.publish(TOPIC.clone(), json.as_bytes());
                }
                EventType::Input(line) => match line.as_str() {
                    "ls peers" => handle_list_peers(&mut swarm).await,
                    cmd if cmd.starts_with("ls books") => handle_list_books(cmd, &mut swarm).await,
                    cmd if cmd.starts_with("add book") => handle_add_book(cmd).await,
                    cmd if cmd.starts_with("share book") => handle_share_book(cmd).await,
                    _ => error!("command unknown"),
                },
            }
        }
    }
}
