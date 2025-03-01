// use std::{net::TcpListener, time::Duration};

// use serde::{Deserialize, Serialize};
// use swarm_lib::protocol::Protocol;

// #[derive(Debug, Serialize, Deserialize)]
// pub enum Message {
//     Ping,
//     Pong,
// }

// // Example usage
// fn main() -> Result<(), ConnectionError> {
//     // Server
//     std::thread::spawn(|| {
//         let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
//         let protocol = Protocol::new();

//         for stream in listener.incoming() {
//             let mut stream = stream.unwrap();
//             let msg: Message = protocol.read_message(&mut stream).unwrap();
//             println!("Server received: {:?}", msg);

//             if let Message::Ping = msg {
//                 protocol.write_message(&mut stream, &Message::Pong).unwrap();
//             }
//         }
//     });

//     std::thread::sleep(Duration::from_millis(100));

//     // Client with 3 retries and 5 second timeout
//     let mut conn =
//         Connection::new("127.0.0.1:8080", 3, Duration::from_secs(5))?;

//     // Send ping
//     conn.send(&Message::Ping)?;

//     // Receive pong
//     let response: Message = conn.receive()?;
//     println!("Client received: {:?}", response);

//     Ok(())
// }

fn main() {}