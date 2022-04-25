# peer2peer

This is an example for building a rather simple peer-to-peer application using the libp2p library.

Start the app with `RUST_LOG=info cargo run`. For testing peer-to-peer connectivity, try using the binary in different folders. Just make sure you have a different `library.json` file for each instance.

Commands to use:
- `ls peers` :  see all peers
- `ls books` :  see local books
- `ls books all` :  see all public/shared books from every peer
- `create book <title>|<author>|<publisher>` :  adds a book to the local library
- `share book <book title>` :  updates a book to be `public :  true`
