use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::debug;

use crate::{
    compute::PhaneronComputeContext,
    graph::{AudioInputId, AudioOutputId, VideoOutputId},
    node_context::{run_node, Node, NodeContext, NodeEvent},
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

pub fn create_phaneron_state(context: PhaneronComputeContext) -> PhaneronState {
    let (node_event_tx, node_event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (state_event_tx, state_event_rx) = tokio::sync::broadcast::channel(10);
    let inner = Arc::new(PhaneronStateInner::new(
        node_event_tx,
        state_event_tx.clone(),
    ));
    tokio::spawn(handle_node_events(
        node_event_rx,
        PhaneronState {
            context: context.clone(),
            inner: inner.clone(),
        },
        state_event_tx,
    ));
    tokio::spawn(handle_state(
        state_event_rx,
        PhaneronState {
            context: context.clone(),
            inner: inner.clone(),
        },
    ));
    PhaneronState { context, inner }
}

#[derive(Clone)]
pub struct PhaneronState {
    context: PhaneronComputeContext,
    inner: Arc<PhaneronStateInner>,
}

impl PhaneronState {
    pub async fn add_node<'a>(
        &self,
        graph_id: &'a GraphId,
        node_id: &'a NodeId,
        context: NodeContext,
        node: Box<dyn Node>,
        name: Option<String>,
    ) {
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
            self.get_node_event_channel().await,
        ));

        self.inner.state_event_tx.send(()).ok();
    }

    pub async fn get_node_event_channel(&self) -> tokio::sync::mpsc::UnboundedSender<NodeEvent> {
        self.inner.node_event_tx.clone()
    }

    pub async fn set_node_name(&self, graph_id: &GraphId, node_id: &NodeId, name: Option<String>) {
        let mut nodes = self.inner.nodes.lock().await;
        let node = nodes.get_mut(node_id).unwrap();
        node.name = name;
    }

    pub async fn set_node_state(&self, graph_id: &GraphId, node_id: &NodeId, state: String) {
        debug!("Setting node {} state to {}", node_id, state);
        let nodes = self.inner.nodes.lock().await;
        let node = nodes.get(node_id).unwrap();
        node.context.set_state(state).await;
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

        let inner_connections = self.inner.video_connections.lock().await.clone();
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

struct PhaneronStateInner {
    graphs: Mutex<HashMap<GraphId, Vec<NodeId>>>,
    nodes: Mutex<HashMap<NodeId, PhaneronStateNode>>,
    node_states: Mutex<HashMap<NodeId, String>>,
    audio_inputs: Mutex<HashMap<NodeId, Vec<AudioInputId>>>,
    audio_outputs: Mutex<HashMap<NodeId, Vec<AudioOutputId>>>,
    video_inputs: Mutex<HashMap<NodeId, Vec<VideoInputId>>>,
    video_outputs: Mutex<HashMap<NodeId, Vec<VideoOutputId>>>,
    video_connections: Mutex<HashMap<VideoInputId, VideoOutputId>>,
    audio_connections: Mutex<HashMap<AudioInputId, AudioOutputId>>,
    subscribers_to_state: Mutex<Vec<tokio::sync::broadcast::Sender<PhaneronStateRepresentation>>>,
    node_event_tx: tokio::sync::mpsc::UnboundedSender<NodeEvent>,
    state_event_tx: tokio::sync::broadcast::Sender<()>,
}

impl PhaneronStateInner {
    fn new(
        node_event_tx: tokio::sync::mpsc::UnboundedSender<NodeEvent>,
        state_event_tx: tokio::sync::broadcast::Sender<()>,
    ) -> Self {
        Self {
            graphs: Default::default(),
            nodes: Default::default(),
            node_states: Default::default(),
            audio_inputs: Default::default(),
            audio_outputs: Default::default(),
            video_inputs: Default::default(),
            video_outputs: Default::default(),
            video_connections: Default::default(),
            audio_connections: Default::default(),
            subscribers_to_state: Default::default(),
            node_event_tx,
            state_event_tx,
        }
    }
}

struct PhaneronStateNode {
    name: Option<String>,
    context: NodeContext,
}

async fn handle_state(
    mut state_event_rx: tokio::sync::broadcast::Receiver<()>,
    state: PhaneronState,
) {
    loop {
        match state_event_rx.recv().await {
            Ok(_) => notify_state(state.clone()).await,
            Err(err) => match err {
                tokio::sync::broadcast::error::RecvError::Closed => return, // No more state
                tokio::sync::broadcast::error::RecvError::Lagged(msgs) => {
                    debug!("handle_state lagged by {msgs} messages");
                }
            },
        }
    }
}

async fn notify_state(state: PhaneronState) {
    let state_representation = state.get_state().await;
    let subscribers_to_state = state.inner.subscribers_to_state.lock().await;
    for sender in subscribers_to_state.iter() {
        sender.send(state_representation.clone()).unwrap(); // TODO: Remove dropped senders
    }
}

async fn handle_node_events(
    mut node_event_rx: tokio::sync::mpsc::UnboundedReceiver<NodeEvent>,
    state: PhaneronState,
    state_event_tx: tokio::sync::broadcast::Sender<()>,
) {
    while let Some(event) = node_event_rx.recv().await {
        let state_modified = match event {
            NodeEvent::StateChanged(node_id, new_state) => {
                node_state_changed(state.clone(), node_id, new_state).await
            }
            NodeEvent::AudioInputAdded(node_id, audio_input_id) => {
                audio_input_added(state.clone(), node_id, audio_input_id).await
            }
            NodeEvent::VideoInputAdded(node_id, video_input_id) => {
                video_input_added(state.clone(), node_id, video_input_id).await
            }
            NodeEvent::AudioOutputAdded(node_id, audio_output_id) => {
                audio_output_added(state.clone(), node_id, audio_output_id).await
            }
            NodeEvent::VideoOutputAdded(node_id, video_output_id) => {
                video_output_added(state.clone(), node_id, video_output_id).await
            }
            NodeEvent::VideoPipeConnected(_node_id, to_video_input, video_output_id) => {
                video_pipe_connected(state.clone(), to_video_input, video_output_id).await
            }
            NodeEvent::AudioPipeConnected(_node_id, to_audio_input, audio_output_id) => {
                audio_pipe_connected(state.clone(), to_audio_input, audio_output_id).await
            }
        };

        if state_modified {
            state_event_tx.send(()).ok();
        }
    }

    // The loop will exit if there are no senders left.
}

async fn node_state_changed(state: PhaneronState, node_id: NodeId, new_state: String) -> bool {
    let mut node_states = state.inner.node_states.lock().await;
    node_states.insert(node_id.clone(), new_state.clone());

    true
}

async fn audio_input_added(
    state: PhaneronState,
    node_id: NodeId,
    audio_input_id: AudioInputId,
) -> bool {
    let mut audio_inputs = state.inner.audio_inputs.lock().await;
    let entry = audio_inputs.entry(node_id).or_default();
    entry.push(audio_input_id);

    true
}

async fn video_input_added(
    state: PhaneronState,
    node_id: NodeId,
    video_input_id: VideoInputId,
) -> bool {
    let mut video_inputs = state.inner.video_inputs.lock().await;
    let entry = video_inputs.entry(node_id.clone()).or_default();
    entry.push(video_input_id);

    true
}

async fn audio_output_added(
    state: PhaneronState,
    node_id: NodeId,
    audio_output_id: AudioOutputId,
) -> bool {
    let mut audio_outputs = state.inner.audio_outputs.lock().await;
    let entry = audio_outputs.entry(node_id).or_default();
    entry.push(audio_output_id);

    true
}

pub async fn video_output_added(
    state: PhaneronState,
    node_id: NodeId,
    video_output_id: VideoOutputId,
) -> bool {
    let mut video_outputs = state.inner.video_outputs.lock().await;
    let entry = video_outputs.entry(node_id.clone()).or_default();
    entry.push(video_output_id);

    true
}

pub async fn video_pipe_connected(
    state: PhaneronState,
    to_video_input: VideoInputId,
    video_pipe_id: VideoOutputId,
) -> bool {
    let mut connections = state.inner.video_connections.lock().await;
    connections.insert(to_video_input, video_pipe_id);

    true
}

pub async fn audio_pipe_connected(
    state: PhaneronState,
    to_audio_input: AudioInputId,
    audio_pipe_id: AudioOutputId,
) -> bool {
    let mut connections = state.inner.audio_connections.lock().await;
    connections.insert(to_audio_input, audio_pipe_id);

    true
}
