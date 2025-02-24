use std::{
    io::stdin,
    sync::mpsc::{Receiver, Sender},
};

use eyre::Result;
use swarm_lib::{
    bot_harness::{run_bots, Bot},
    Action,
    BotMsg,
    BotMsgEnvelope,
    Query,
    QueryEnvelope,
    ResponseEnvelope,
};

fn main() -> Result<()> {
    run_bots::<SimpleBot>()
}

struct SimpleBot {
    bot_id: u32,
    resp_rx: Receiver<ResponseEnvelope>,
    bot_msg_tx: Sender<BotMsgEnvelope>,
    query_seq: u32,
}

impl Bot for SimpleBot {
    fn new(
        bot_id: u32,
        resp_rx: Receiver<ResponseEnvelope>,
        bot_msg_tx: Sender<BotMsgEnvelope>,
    ) -> Self {
        Self {
            bot_id,
            resp_rx,
            bot_msg_tx,
            query_seq: 0,
        }
    }

    fn run(self) -> Result<()> {
        loop {
            let mut input = String::new();
            stdin().read_line(&mut input).unwrap();

            match input.trim() {
                "move" => {
                    self.bot_msg_tx.send(BotMsgEnvelope {
                        bot_id: self.bot_id,
                        msg: BotMsg::Action(Action::Move(1, 0)),
                    })?;
                }
                "radar" => {
                    let query = QueryEnvelope {
                        query_seq: self.query_seq,
                        query: Query::GetRadar,
                    };
                    self.bot_msg_tx.send(BotMsgEnvelope {
                        bot_id: self.bot_id,
                        msg: BotMsg::Query(query),
                    })?;

                    // Wait for response
                    if let Ok(response) = self.resp_rx.recv() {
                        println!("Got radar response: {:?}", response);
                    }
                }
                "this" => {
                    let query = QueryEnvelope {
                        query_seq: self.query_seq,
                        query: Query::GetThis,
                    };
                    self.bot_msg_tx.send(BotMsgEnvelope {
                        bot_id: self.bot_id,
                        msg: BotMsg::Query(query),
                    })?;

                    // Wait for response
                    if let Ok(response) = self.resp_rx.recv() {
                        println!("Got this response: {:?}", response);
                    }
                }
                _ => continue,
            };
        }
    }
}
