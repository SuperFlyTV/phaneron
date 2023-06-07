use serde::{Deserialize, Serialize};

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
