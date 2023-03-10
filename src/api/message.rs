use serde::{Deserialize, Serialize};

use crate::state::PhaneronStateRepresentation;

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    #[serde(rename = "userId")]
    pub user_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "event")]
pub enum ClientEvent {
    Topics(TopicsRequest),
    NodeState(NodeStateRequest),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TopicsRequest {
    pub topics: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeStateRequest {
    pub node_id: String,
    pub state: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlipperState {
    pub flipped: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerEvent {
    PhaneronState(PhaneronStateRepresentation),
}
