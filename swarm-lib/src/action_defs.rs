use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{BotResp, Dir, Energy, Pos};

pub type ActionId = u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionWithId {
    pub id: ActionId,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    MoveDir(Dir),
    MoveTo(Pos),
    Harvest(Dir),
}

impl Action {
    pub fn energy(&self) -> Option<Energy> {
        Some(
            match self {
                Action::MoveDir(_) => 1,
                Action::MoveTo(_) => return None,
                Action::Harvest(_) => 5,
            }
            .into(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action: Action,
    pub id: ActionId,
    pub status: ActionStatus,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    strum_macros::EnumDiscriminants,
)]
pub enum ActionStatus {
    Success,
    Failure(String),
    InProgress { progress: u16, total: u16 },
}

/// Builder for BotResponse to enable fluent method chaining
#[derive(Debug, Clone, Default)]
pub struct RespBuilder {
    pub actions: Vec<ActionWithId>,
    pub cancel_actions: Vec<ActionId>,
    pub cancel_all_actions: bool,
}

impl RespBuilder {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            cancel_actions: Vec::new(),
            cancel_all_actions: false,
        }
    }

    pub fn push_action_id(
        &mut self,
        action: Action,
        id: ActionId,
    ) -> &mut Self {
        self.actions.push(ActionWithId { id, action });
        self
    }

    pub fn push_action(&mut self, action: Action) -> &mut Self {
        self.push_action_id(action, rand::rng().random())
    }

    pub fn cancel_action(&mut self, id: ActionId) -> &mut Self {
        self.cancel_actions.push(id);
        self
    }

    pub fn cancel_all_actions(&mut self) -> &mut Self {
        self.cancel_all_actions = true;
        self
    }

    pub fn build(&mut self) -> BotResp {
        BotResp {
            actions: std::mem::take(&mut self.actions),
            cancel_actions: std::mem::take(&mut self.cancel_actions),
            cancel_all_actions: self.cancel_all_actions,
        }
    }
}
