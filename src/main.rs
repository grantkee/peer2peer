use tokio::{sync::mpsc};
use serde::{Deserialize, Serialize};
use log::{info, error};
use libp2p::{
    floodsub::{Floodsub, Topic},
    identity,
    noise::{Keypair, X25519Spec},
    NetworkBehaviour, PeerId, Transport,
};
use once_cell::sync::Lazy;


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

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    info!("Peer Id: {}", PEER_ID.clone());

    // multi-producer, single-consumer queue for sending values across asynchronous tasks.
    // aka - async channel for communicating between different parts of the application
    let (response_sender, mut response_receiver) = mpsc::unbounded_channel();

    // authentication keys using noise protocol
    let auth_keys = Keypair::<X25519Spec>::new().into_authentic(&KEYS).expect("unable to create auth keys");
}
