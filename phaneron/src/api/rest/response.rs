use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::state::PhaneronNodeRepresentation;

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginNotFound404Response {
    pub not_found: String, // const: 'plugin'
    pub message: String,
}

impl PluginNotFound404Response {
    pub fn new(plugin_id: String) -> Self {
        Self {
            not_found: "plugin".to_string(),
            message: format!("Plugin {plugin_id} does not exist"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNotFound404Response {
    pub not_found: String, // const: 'graph'
    pub message: String,
}

impl GraphNotFound404Response {
    pub fn new(graph_id: String) -> Self {
        Self {
            not_found: "graph".to_string(),
            message: format!("Graph {graph_id} does not exist"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeNotFound404Response {
    pub not_found: String, // const: 'node'
    pub message: String,
}

impl NodeNotFound404Response {
    pub fn new(node_id: String) -> Self {
        Self {
            not_found: "node".to_string(),
            message: format!("Node {node_id} does not exist"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputNotFound404Response {
    pub not_found: String, // const: 'node'
    pub message: String,
}

impl InputNotFound404Response {
    pub fn new(input_id: String) -> Self {
        Self {
            not_found: "input".to_string(),
            message: format!("Input {input_id} does not exist"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeTypeNotFound404Response {
    pub not_found: String, // const: 'node_type'
    pub node_type: String,
}

impl NodeTypeNotFound404Response {
    pub fn new(node_type: String) -> Self {
        Self {
            not_found: "node_type".to_string(),
            node_type,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeTypeDoesNotMatch409Response {
    pub existing_node_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputTypeDoesNotMatch409Response {
    pub input_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPhaneronState200Response {
    pub nodes: HashMap<String, PhaneronStateNode>,
    pub audio_outputs: HashMap<String, Vec<PhaneronStateNodeAudioOutput>>,
    pub audio_inputs: HashMap<String, Vec<PhaneronStateNodeAudioInput>>,
    pub video_outputs: HashMap<String, Vec<PhaneronStateNodeVideoOutput>>,
    pub video_inputs: HashMap<String, Vec<PhaneronStateNodeVideoInput>>,
    pub connections: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaneronStateNode {
    pub name: String,
    pub state: String,
    pub configuration: Option<String>,
    pub node_type: String,
}

impl From<PhaneronNodeRepresentation> for PhaneronStateNode {
    fn from(value: PhaneronNodeRepresentation) -> Self {
        PhaneronStateNode {
            name: value.name,
            state: value.state,
            configuration: value.configuration,
            node_type: value.node_type,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhaneronStateNodeAudioOutput {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhaneronStateNodeAudioInput {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhaneronStateNodeVideoOutput {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhaneronStateNodeVideoInput {
    pub id: String,
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
pub struct GetGraph200Response {
    pub id: String,
    pub name: String,
    pub nodes: Vec<GetGraph200ResponseNode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetGraph200ResponseNode {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetGraphNodes200Response {
    pub nodes: Vec<PhaneronGraphNode>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetNodeInputConnection200Response {
    pub id: String,
    pub input_type: String,
    pub connected_output_id: Option<String>,
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
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaneronGraphNode {
    pub id: String,
    pub name: String,
    pub node_type: String,
    pub state: String,
    pub configuration: Option<String>,
    pub audio_inputs: Vec<PhaneronGraphNodeAudioInput>,
    pub video_inputs: Vec<PhaneronGraphNodeVideoInput>,
    pub audio_outputs: Vec<PhaneronGraphNodeAudioOutput>,
    pub video_outputs: Vec<PhaneronGraphNodeVideoOutput>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhaneronGraphNodeAudioInput {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhaneronGraphNodeAudioOutput {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhaneronGraphNodeVideoInput {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhaneronGraphNodeVideoOutput {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetNodeState200Response {
    pub state: String,
    pub configuration: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetGraphNodeInputs200Response {
    pub inputs: Vec<GraphNodeInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNodeInput {
    pub id: String,
    pub input_type: String,
    pub connected_output_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetGraphNodeOutputs200Response {
    pub outputs: Vec<GraphNodeOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNodeOutput {
    pub id: String,
    pub output_type: String,
    pub connected_input_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddGraph200Response {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddGraphNode200Response {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddOrUpdateGraphNode200Response {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectGrahNodeInput200Response {
    pub id: String,
    pub input_type: String,
    pub connected_output_id: Option<String>,
}
