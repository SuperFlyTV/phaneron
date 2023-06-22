/*
 * Phaneron media compositing software.
 * Copyright (C) 2023 SuperFlyTV AB
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use serde::{Deserialize, Serialize};

use crate::state::PhaneronStateRepresentation;

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
    pub graph_id: String,
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
