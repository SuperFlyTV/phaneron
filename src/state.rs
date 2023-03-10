use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::debug;

use crate::{
    compute::PhaneronComputeContext,
    graph::{AudioInputId, AudioOutputId, VideoOutputId},
    node_context::{run_node, Node, NodeContext},
    GraphId, NodeId, VideoInputId,
};

// Representation of the state that is safe to expose to the outside world
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaneronStateRepresentation {
    pub nodes: HashMap<String, PhaneronNodeRepresentation>,
    pub video_outputs: HashMap<String, Vec<String>>,
    pub video_inputs: HashMap<String, Vec<String>>,
    pub connections: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaneronNodeRepresentation {
    name: Option<String>,
    state: Option<String>,
}

#[derive(Clone)]
pub struct PhaneronState {
    context: PhaneronComputeContext,
    inner: Arc<PhaneronStateInner>,
}

impl PhaneronState {
    pub fn new(context: PhaneronComputeContext) -> Self {
        Self {
            context,
            inner: Default::default(),
        }
    }

    pub async fn add_node<'a>(
        &self,
        graph_id: &'a GraphId,
        node_id: &'a NodeId,
        context: NodeContext,
        node: Box<dyn Node>,
        name: Option<String>,
    ) {
        {
            let mut graphs = self.inner.graphs.lock().await;
            let graph_entry = graphs.entry(graph_id.clone()).or_default();
            graph_entry.push(node_id.clone());

            let mut nodes = self.inner.nodes.lock().await;
            nodes.insert(
                node_id.clone(),
                PhaneronStateNode {
                    name,
                    context: context.clone(),
                },
            );

            let pending_state_channel = context.get_pending_state_channel();

            tokio::spawn(run_node(
                self.context.clone(),
                context,
                node,
                pending_state_channel,
            ));
        }

        self.notify_state().await;
    }

    pub async fn set_node_name(&self, graph_id: &GraphId, node_id: &NodeId, name: Option<String>) {
        {
            let mut nodes = self.inner.nodes.lock().await;
            let node = nodes.get_mut(node_id).unwrap();
            node.name = name;
        }

        self.notify_state().await;
    }

    pub async fn set_node_state(&self, graph_id: &GraphId, node_id: &NodeId, state: String) {
        {
            debug!("Setting node {} state to {}", node_id, state);
            let nodes = self.inner.nodes.lock().await;
            let node = nodes.get(node_id).unwrap();
            let mut node_states = self.inner.node_states.lock().await;
            node_states.insert(node_id.clone(), state.clone()); // TODO: Bad! Assumes the state will be applied. The node should emit an event for this.
            node.context.set_state(state).await;
        }

        self.notify_state().await;
    }

    pub async fn subscribe(&self) -> tokio::sync::broadcast::Receiver<PhaneronStateRepresentation> {
        let (sender, receiver) = tokio::sync::broadcast::channel(1); // Only the latest value is relevant

        {
            let state = self.get_state().await;
            sender.send(state).unwrap();
        }
        self.inner.subscribers_to_state.lock().await.push(sender);

        receiver
    }

    #[deprecated]
    pub async fn audio_input_added(&self, node_id: &NodeId, audio_input_id: AudioInputId) {
        {
            let mut audio_inputs = self.inner.audio_inputs.lock().await;
            let entry = audio_inputs.entry(node_id.clone()).or_default();
            entry.push(audio_input_id);
        }

        self.notify_state().await;
    }

    #[deprecated]
    pub async fn audio_output_added(&self, node_id: &NodeId, audio_output_id: AudioOutputId) {
        {
            let mut audio_outputs = self.inner.audio_outputs.lock().await;
            let entry = audio_outputs.entry(node_id.clone()).or_default();
            entry.push(audio_output_id);
        }

        self.notify_state().await;
    }

    #[deprecated]
    pub async fn video_input_added(&self, node_id: &NodeId, video_input_id: VideoInputId) {
        {
            let mut video_inputs = self.inner.video_inputs.lock().await;
            let entry = video_inputs.entry(node_id.clone()).or_default();
            entry.push(video_input_id);
        }

        self.notify_state().await;
    }

    #[deprecated]
    pub async fn video_output_added(&self, node_id: &NodeId, video_output_id: VideoOutputId) {
        {
            let mut video_outputs = self.inner.video_outputs.lock().await;
            let entry = video_outputs.entry(node_id.clone()).or_default();
            entry.push(video_output_id);
        }

        self.notify_state().await;
    }

    #[deprecated]
    pub async fn video_pipe_connected(
        &self,
        to_video_input: VideoInputId,
        video_pipe_id: VideoOutputId,
    ) {
        {
            let mut connections = self.inner.connections.lock().await;
            connections.insert(to_video_input, video_pipe_id);
        }

        self.notify_state().await;
    }

    #[deprecated]
    async fn notify_state(&self) {
        let state = self.get_state().await;
        let subscribers_to_state = self.inner.subscribers_to_state.lock().await;
        for sender in subscribers_to_state.iter() {
            sender.send(state.clone()).unwrap(); // TODO: Remove dropped senders
        }
    }

    async fn get_state(&self) -> PhaneronStateRepresentation {
        let mut nodes = HashMap::new();
        let mut video_outputs = HashMap::new();
        let mut video_inputs = HashMap::new();
        let mut connections = HashMap::new();

        let inner_node_states = self.inner.node_states.lock().await.clone();
        for (node_id, node) in self.inner.nodes.lock().await.iter() {
            let node_state = inner_node_states.get(node_id);
            nodes.insert(
                node_id.to_string(),
                PhaneronNodeRepresentation {
                    name: node.name.clone(),
                    state: node_state.cloned(),
                },
            );
        }

        let inner_video_outputs = self.inner.video_outputs.lock().await.clone();
        for (node_id, output) in inner_video_outputs.iter() {
            video_outputs.insert(
                node_id.to_string(),
                output.iter().map(|o| o.to_string()).collect(),
            );
        }

        let inner_video_inputs = self.inner.video_inputs.lock().await.clone();
        for (node_id, output) in inner_video_inputs.iter() {
            video_inputs.insert(
                node_id.to_string(),
                output.iter().map(|o| o.to_string()).collect(),
            );
        }

        let inner_connections = self.inner.connections.lock().await.clone();
        for (input, output) in inner_connections.iter() {
            connections.insert(input.to_string(), output.to_string());
        }

        PhaneronStateRepresentation {
            nodes,
            video_outputs,
            video_inputs,
            connections,
        }
    }
}

#[derive(Default)]
struct PhaneronStateInner {
    graphs: Mutex<HashMap<GraphId, Vec<NodeId>>>,
    nodes: Mutex<HashMap<NodeId, PhaneronStateNode>>,
    node_states: Mutex<HashMap<NodeId, String>>,
    audio_inputs: Mutex<HashMap<NodeId, Vec<AudioInputId>>>,
    audio_outputs: Mutex<HashMap<NodeId, Vec<AudioOutputId>>>,
    video_inputs: Mutex<HashMap<NodeId, Vec<VideoInputId>>>,
    video_outputs: Mutex<HashMap<NodeId, Vec<VideoOutputId>>>,
    connections: Mutex<HashMap<VideoInputId, VideoOutputId>>,
    subscribers_to_state: Mutex<Vec<tokio::sync::broadcast::Sender<PhaneronStateRepresentation>>>,
}

struct PhaneronStateNode {
    name: Option<String>,
    context: NodeContext,
}
