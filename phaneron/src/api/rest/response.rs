use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginNotFound404Response {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetAvailablePlugins200Response {
    pub plugins: Vec<PluginDescription>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginDescription {
    pub id: String,
    pub nodes: Vec<PluginNodeDescription>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginNodeDescription {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetAvailablePluginNodes200Response {
    pub nodes: Vec<AvailablePluginNode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AvailablePluginNode {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetGraphs200Response {
    pub graphs: Vec<GraphDescription>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphDescription {
    pub id: String,
    pub name: String,
    pub nodes: Vec<GraphNodeDescription>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphNodeDescription {
    pub id: String,
    pub name: Option<String>,
}
