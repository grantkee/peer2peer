use crate::ListResponse;

use super::{Book, BookBehavior, Library, ListMode, ListRequest, STORAGE_PATH, TOPIC};
use libp2p::swarm::Swarm;
use log::{error, info};
use tokio::{fs, sync::mpsc};

async fn read_local_library() -> Result<Library> {
    let content = fs::read(STORAGE_PATH).await?;
    let result = serde_json::from_slice(&content)?;
    Ok(result)
}

async fn write_local_library(library: &Library) -> Result<()> {
    let json = serde_json::to_string(&library)?;
    fs::write(STORAGE_PATH, &json).await?;
    Ok(())
}

pub async fn handle_list_peers(swarm: &mut Swarm<BookBehavior>) {
    info!("Peers discovered: ");
    let nodes = swarm.mdns.discovered_nodes();
    let mut unique_peers = std::collections::HashSet::new();

    for peer in nodes {
        unique_peers.insert(peer);
    }

    unique_peers.iter().for_each(|p| info!("{}", p))
}

pub async fn handle_add_book(cmd: &str) {
    if let Some(input) = cmd.strip_prefix("add book") {
        let elem: Vec<&str> = input.split("|").collect();
        if elem.len() < 3 {
            info!("missing arguments. format should be: title|author|publisher")
        } else {
            let title = elem.get(0).expect("unable to get title");
            let author = elem.get(1).expect("unable to get author");
            let publisher = elem.get(2).expect("unable to get publisher");
            if let Err(e) = add_new_book(title, author, publisher) {
                error!("error adding book to library: {}", e);
            }
        }
    }
}

async fn add_new_book(title: &str, author: &str, publisher: &str) -> Result<()> {
    let mut local_library = read_local_library().await?;
    let next_id = match local_library.iter().max_by_key(|book| book.id) {
        Some(val) => val.id + 1,
        None => 0,
    };
    local_library.push(Book {
        id: next_id,
        title: title.to_owned(),
        author: author.to_owned(),
        publisher: publisher.to_owned(),
        public: false,
    });
    write_local_library(&local_library).await?;
    info!(
        "added book: {} by {} - published by {}",
        title, author, publisher
    );

    Ok(())
}

pub async fn handle_share_book(cmd: &str) {
    if let Some(input) = cmd.strip_prefix("share book") {
        match input.trim() {
            Ok(title) => {
                if let Err(e) = share_book(title).await {
                    info!("error sharing book {}: {}", title, e);
                } else {
                    info!("now sharing book: {}", title);
                }
            }
            Err(e) => error!("invaltitle title: {}, {}", input.trim(), e),
        };
    }
}

async fn share_book(title: &str) -> Result<()> {
    let mut local_library = read_local_library().await?;
    local_library
        .iter_mut()
        .filter(|book| book.title == title)
        .for_each(|b| b.public = true);
    write_local_library(&local_library).await?;
    Ok(())
}

pub async fn handle_list_books(cmd: &str, swarm: &mut Swarm<BookBehavior>) {
    let input = cmd.strip_prefix("ls books");

    match input {
        Some("all") => {
            let req = ListRequest {
                mode: ListMode::ALL,
            };
            let json = serde_json::to_string(&req).expect("unable to jsonify request for all");
            swarm.floodsub.publish(TOPIC.clone(), json.as_bytes());
        }
        Some(library_peer_id) => {
            let req = ListRequest {
                mode: ListMode::One(library_peer_id.to_owned()),
            };
            let json =
                serde_json::to_string(&req).expect("unable to jsonify request for library peer id");
            swarm.floodsub.publish(TOPIC.clone(), json.as_bytes());
        }
        None => {
            match read_local_library().await {
                Ok(val) => {
                    info!("Local books ({})", val.len());
                    val.iter().for_each(|book| info!("{:?}", book));
                }
                Err(e) => error!("error retrieving local library: {}", e),
            };
        }
    }
}

pub async fn respond_with_public_books(
    sender: mpsc::UnboundedSender<ListResponse>,
    receiver: String,
) {
    tokio::spawn(async move {
        match read_local_library().await {
            Ok(books) => {
                let res = ListResponse {
                    mode: ListMode::ALL,
                    receiver,
                    data: books.into_iter().filter(|b| b.public).collect(),
                };
                if let Err(e) = sender.send(res) {
                    error!("error responding: {}", e);
                }
            }
            Err(e) => error!("error retrieving local library: {}", e),
        }
    });
}
