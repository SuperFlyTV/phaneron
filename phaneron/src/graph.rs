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

use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GraphId(String);
impl GraphId {
    pub fn new_from(id: String) -> Self {
        Self(id)
    }
}
impl Default for GraphId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
impl Display for GraphId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(String);
impl NodeId {
    pub fn new_from(id: String) -> Self {
        Self(id)
    }
}
impl Default for NodeId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
impl Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
