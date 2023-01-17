//! early interface of socket server
//! TODO: event based programming :)

extern crate tokio;
extern crate futures_channel;
extern crate futures_util;
extern crate tokio_tungstenite;

use std::{
    collections::HashMap,
    env,
    io::Error as IoError,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, pin_mut, stream::TryStreamExt, StreamExt};

use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::protocol::Message;

type PeerMap = Arc<Mutex<HashMap<SocketAddr, UnboundedSender<Message>>>>;

#[derive(Debug, Clone)]
pub struct SocketServer {
    peer_map: PeerMap,
}

impl SocketServer {
    pub fn new() -> Self {
        Self {
            peer_map: PeerMap::new(Mutex::new(HashMap::new())),
        }
    }

    async fn handle_connection(peer_map: PeerMap, addr: SocketAddr, stream: TcpStream) {
        println!("Incoming TCP connection from: {}", addr);
    
        let ws_stream = tokio_tungstenite::accept_async(stream)
            .await
            .expect("Error during the websocket handshake occurred");
        println!("WebSocket connection established: {}", addr);

        let (source, server) = unbounded();
        peer_map.lock().unwrap().insert(addr, source);
    
        let (outgoing, incoming) = ws_stream.split();
    
        let broadcast_incoming = incoming.try_for_each(|msg| {
            println!("Received a message from {}: {}", addr, msg.to_text().unwrap());
            
            if let Some(response) = peer_map.lock().unwrap().get(&addr) {
                response.unbounded_send(Message::Text(msg.to_string().split(" ").last().unwrap_or_default().to_string())).unwrap();
            } else {
                eprintln!("source not found");
            }

            future::ok(())
        });
    
        let receive_from_others = server.map(Ok).forward(outgoing);
    
        pin_mut!(broadcast_incoming, receive_from_others);
        future::select(broadcast_incoming, receive_from_others).await;
    
        println!("{} disconnected", &addr);
        peer_map.lock().unwrap().remove(&addr);
    }

    pub async fn listen(self) {
        let _self = Arc::new(self);
        let addr = env::args().nth(1).unwrap_or_else(|| "127.0.0.1:8080".to_string());
        let try_socket = TcpListener::bind(&addr).await;
        let listener = try_socket.expect("failed to bind");
        println!("Listening on: {}", addr);


        while let Ok((stream, addr)) = listener.accept().await {
            let peer_map = _self.peer_map.clone();
            tokio::spawn(SocketServer::handle_connection(peer_map, addr, stream));
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), IoError> {
    let server = SocketServer::new();

    server.listen().await;

    Ok(())
}
