use crate::commands::{handle_add_book, handle_list_books, handle_list_peers, handle_share_book};
use libp2p::{
    core::upgrade,
    floodsub::{Floodsub, Topic},
    identity,
    mdns::{Mdns, MdnsEvent},
    mplex,
    noise::{Keypair, NoiseConfig, X25519Spec},
    swarm::{Swarm, SwarmBuilder},
    tcp::TokioTcpConfig,
    NetworkBehaviour, PeerId, Transport,
};
use log::{error, info};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
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
struct ListResponse {
    mode: ListMode,
    data: Library,
    receiver: String,
}

enum EventType {
    Response(ListResponse),
    Input(String),
}

#[derive(NetworkBehaviour)]
struct BookBehavior {
    floodsub: Floodsub,
    mdns: Mdns,
    #[behaviour(ignore)]
    response_sender: mpsc::UnboundedSender<ListResponse>,
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
                event = swarm.next() => {
                    info!("Unhandled swarm event: {:?}", event);
                    None
                },
                response = response_receiver.recv() => Some(EventType::Response(response.expect("unable to get response")))
            }
        };

        if let Some(event) = event_type {
            match event {
                EventType::Response(res) => {
                    unimplemented!()
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
