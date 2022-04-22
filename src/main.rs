use serde::{Deserialize, Serialize};
type Library = Vec<Book>;

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
    println!("Hello, world!");
}
