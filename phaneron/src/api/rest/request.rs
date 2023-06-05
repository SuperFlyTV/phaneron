use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    #[serde(rename = "userId")]
    pub user_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetNodeStateSchemaParams {
    #[serde(rename = "pluginId")]
    pub plugin_id: String,
    #[serde(rename = "nodeId")]
    pub node_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddGraphRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetGraphNodeParams {
    #[serde(rename = "graphId")]
    pub graph_id: String,
    #[serde(rename = "nodeId")]
    pub node_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetGraphNodeStateParams {
    #[serde(rename = "graphId")]
    pub graph_id: String,
    #[serde(rename = "nodeId")]
    pub node_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetGraphNodeInputsParams {
    #[serde(rename = "graphId")]
    pub graph_id: String,
    #[serde(rename = "nodeId")]
    pub node_id: String,
    #[serde(rename = "inputId")]
    pub input_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetGraphNodeInputConnectionsParams {
    #[serde(rename = "graphId")]
    pub graph_id: String,
    #[serde(rename = "nodeId")]
    pub node_id: String,
    #[serde(rename = "inputId")]
    pub input_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectGraphNodeInputParams {
    #[serde(rename = "graphId")]
    pub graph_id: String,
    #[serde(rename = "nodeId")]
    pub node_id: String,
    #[serde(rename = "inputId")]
    pub input_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectGraphNodeInputRequest {
    #[serde(rename = "graphId")]
    pub connect_from_node_id: String,
    pub connect_from_output_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DisconnectGraphNodeInputParams {
    #[serde(rename = "graphId")]
    pub graph_id: String,
    #[serde(rename = "nodeId")]
    pub node_id: String,
    #[serde(rename = "inputId")]
    pub input_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetGraphNodeOutputsParams {
    #[serde(rename = "graphId")]
    pub graph_id: String,
    #[serde(rename = "nodeId")]
    pub node_id: String,
}
