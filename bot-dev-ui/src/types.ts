// TypeScript definitions for Rust types

export interface JournalEntry {
  timestamp: string;
  bot_id?: number;
  client_msg?: ClientMsg;
  server_msg?: ServerMsg;
}

// Client Messages
export type ClientMsg = { Connect: null } | { BotMsg: BotMsgEnvelope };

export interface BotMsgEnvelope {
  bot_id: number;
  tick: number;
  msg: BotResponse;
}

export interface BotResponse {
  actions: ActionEnvelope[];
  subscribe: SubscriptionType[];
  unsubscribe: SubscriptionType[];
}

// Server Messages
export type ServerMsg =
  | { ConnectAck: null }
  | { AssignBot: [number, string] }
  | { ServerUpdate: ServerUpdateEnvelope }
  | { Close: null };

export interface ServerUpdateEnvelope {
  bot_id: number;
  seq: number;
  response: ServerUpdate;
}

export interface ServerUpdate {
  tick: number;
  team?: Team;
  position?: Pos;
  radar?: RadarData;
  items?: Record<string, number>;
  action_result?: ActionResult;
}

// Action Types
export interface ActionEnvelope {
  id: number;
  action: Action;
}

export type Action = { MoveDir: Dir } | { MoveTo: Pos };

export interface ActionResult {
  action: Action;
  id: number;
  status: ActionStatus;
}

export type ActionStatus = "Success" | "Failure" | "InProgress";

// Common Types
export type Dir = "Up" | "Down" | "Left" | "Right";
export type Team = "Player" | "Enemy";
export type SubscriptionType = "Position" | "Radar" | "Team";

export type Pos = [number, number];

// Placeholder for RadarData - adjust based on actual implementation
export interface RadarData {
  entities: RadarEntity[];
}

export interface RadarEntity {
  position: Pos;
  team?: Team;
  type: string;
}
