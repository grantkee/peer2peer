use super::{BookBehavior, Library, STORAGE_PATH, Book};
use tokio::fs;
use log::{error, info};
use libp2p::{swarm::{Swarm}};

pub async fn handle_list_peers(swarm: &mut Swarm<BookBehavior>) {
    info!("Peers discovered: ");
    let nodes = swarm.mdns.discovered_nodes();
    let mut unique_peers = HashSet::new();

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
    info!("added book: {} by {} - published by {}", title, author, publisher);

    Ok(())
}



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
