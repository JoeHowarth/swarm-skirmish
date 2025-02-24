use bevy_ecs::{component::Component, entity::Entity};
use serde::{Deserialize, Serialize};

pub mod bot_harness;
pub mod protocol;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMsg {
    Connect,
    BotMsg(BotMsgEnvelope),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotMsgEnvelope {
    pub bot_id: u32,
    pub msg: BotMsg,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BotMsg {
    Action(Action),
    Query(QueryEnvelope),
}

#[derive(Debug, Clone, Serialize, Deserialize)] 
pub enum Action {
    Move(i8, i8),
    // WaitUntilTick(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Query {
    GetRadar,
    GetThis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEnvelope {
    pub query_seq: u32,
    pub query: Query,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMsg {
    ConnectAck,
    AssignBot(u32),
    Response(ResponseEnvelope),
    Close,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub bot_id: u32,
    pub tick: u32,
    pub query_seq: u32,
    pub response: Response,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    RadarResponse(Vec<(usize, usize, RadarCellState)>),
    ThisResponse(ThisResponse),
    Ack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThisResponse {
    pub x: u32,
    pub y: u32,
    pub team: Team,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RadarCellState {
    Empty,
    Blocked,
    Pawn { team: Team, entity: Entity },
}

#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Team {
    Player,
    Enemy,
}
