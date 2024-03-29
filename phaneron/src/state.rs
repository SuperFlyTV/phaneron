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

use std::{collections::HashMap, sync::Arc};

use abi_stable::std_types::ROption::{RNone, RSome};
use phaneron_plugin::{
    types::Node, types::NodeHandle, AudioInputId, AudioOutputId, VideoInputId, VideoOutputId,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc::UnboundedReceiver, Mutex};
use tracing::debug;

use crate::{
    channel::ChannelSemaphoreProvider,
    compute::PhaneronComputeContext,
    node_context::{
        apply_node_state, create_node_context, handle_node_event, run_node, NodeEvent,
        NodeRunContext, NodeStateEvent,
    },
    plugins::PluginManager,
    GraphId, NodeId,
};

/// Representation of the state that is safe to expose to the outside world
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

pub struct CreateNode {
    pub node_id: String,
    pub node_type: String,
    pub node_name: Option<String>,
    pub state: Option<String>,
    pub configuration: Option<String>,
}

pub enum CreateConnectionType {
    Video,
    Audio,
}

pub struct CreateConnection {
    pub connection_type: CreateConnectionType,
    pub from_node_id: String,
    pub from_output_index: usize,
    pub to_node_id: String,
    pub to_input_index: usize,
}

#[derive(Clone)]
pub struct PhaneronState {
    context: PhaneronComputeContext,
    inner: Arc<PhaneronStateInner>,
}

impl PhaneronState {
    pub async fn create_graph(
        &self,
        plugin_manager: &PluginManager,
        graph_id: &GraphId,
        nodes: Vec<CreateNode>,
        connections: Vec<CreateConnection>,
    ) -> anyhow::Result<()> {
        let mut created_node_handles: Vec<(NodeId, NodeHandle)> = vec![];
        let mut node_configurations: HashMap<NodeId, String> = HashMap::new();
        for create_node in nodes.iter() {
            let node = plugin_manager
                .create_node_handle(create_node.node_id.clone(), create_node.node_type.clone())
                .unwrap(); // TODO: Don't panic!
            let node_id = NodeId::new_from(create_node.node_id.clone());
            created_node_handles.push((node_id.clone(), node));
            if let Some(config) = &create_node.configuration {
                node_configurations.insert(node_id, config.clone());
            }
        }

        let mut initialzed_nodes: HashMap<
            NodeId,
            (
                Node,
                NodeRunContext,
                UnboundedReceiver<NodeEvent>,
                ChannelSemaphoreProvider,
            ),
        > = HashMap::new();
        for (node_id, handle) in created_node_handles {
            let (node_context, node_run_context, state_rx, semaphore_provider) =
                create_node_context(
                    self.context.clone(),
                    node_id.clone(),
                    self.get_node_event_channel().await,
                )
                .await;
            let (sender, receiver) = tokio::sync::oneshot::channel();
            let configuration = node_configurations.remove(&node_id);
            std::thread::spawn(move || {
                let configuration = match configuration {
                    Some(config) => RSome(config.into()),
                    None => RNone,
                };
                let node = handle.initialize(node_context, configuration);
                sender.send(node).ok();
            });
            let node = receiver.await.unwrap();
            initialzed_nodes.insert(
                node_id,
                (node, node_run_context, state_rx, semaphore_provider),
            );
        }

        for create_node in nodes {
            let node_id = NodeId::new_from(create_node.node_id.clone());
            let (node, run_context, node_event_rx, semaphore_provider) =
                initialzed_nodes.remove(&node_id).unwrap();
            let node = Arc::new(node);
            if let Some(state) = create_node.state {
                apply_node_state(
                    node_id.clone(),
                    node.clone(),
                    state,
                    self.inner.node_event_tx.clone(),
                )
                .await;
            }
            self.add_node(
                graph_id,
                &node_id,
                create_node.node_name,
                run_context,
                node,
                node_event_rx,
                semaphore_provider,
            )
            .await;
        }

        for connection in connections {
            match connection.connection_type {
                CreateConnectionType::Video => {
                    let output = {
                        let video_outputs = self.inner.video_outputs.lock().await;
                        let from_node_outputs = video_outputs
                            .get(&NodeId::new_from(connection.from_node_id.clone()))
                            .unwrap();
                        from_node_outputs
                            .get(connection.from_output_index)
                            .unwrap()
                            .clone()
                    };
                    let input = {
                        let video_inputs = self.inner.video_inputs.lock().await;
                        let to_node_inputs = video_inputs
                            .get(&NodeId::new_from(connection.to_node_id.clone()))
                            .unwrap();
                        to_node_inputs
                            .get(connection.to_input_index)
                            .unwrap()
                            .clone()
                    };

                    let video_pipe = {
                        let nodes_lock = self.inner.nodes.lock().await;
                        let from_node = nodes_lock
                            .get(&NodeId::new_from(connection.from_node_id.clone()))
                            .unwrap();
                        from_node.context.get_video_pipe(&output).await
                    };

                    let to_node_context = {
                        let nodes_lock = self.inner.nodes.lock().await;
                        let to_node = nodes_lock
                            .get(&NodeId::new_from(connection.to_node_id.clone()))
                            .unwrap();
                        to_node.context.clone()
                    };

                    to_node_context
                        .connect_video_pipe(&input, video_pipe)
                        .await
                        .unwrap();

                    video_pipe_connected(
                        PhaneronState {
                            context: self.context.clone(),
                            inner: self.inner.clone(),
                        },
                        input,
                        output,
                    )
                    .await;
                }
                CreateConnectionType::Audio => {
                    let output = {
                        let audio_outputs = self.inner.audio_outputs.lock().await;
                        let from_node_outputs = audio_outputs
                            .get(&NodeId::new_from(connection.from_node_id.clone()))
                            .unwrap();
                        from_node_outputs
                            .get(connection.from_output_index)
                            .unwrap()
                            .clone()
                    };
                    let input = {
                        let audio_inputs = self.inner.audio_inputs.lock().await;
                        let to_node_inputs = audio_inputs
                            .get(&NodeId::new_from(connection.to_node_id.clone()))
                            .unwrap();
                        to_node_inputs
                            .get(connection.to_input_index)
                            .unwrap()
                            .clone()
                    };

                    let audio_pipe = {
                        let nodes_lock = self.inner.nodes.lock().await;
                        let from_node = nodes_lock
                            .get(&NodeId::new_from(connection.from_node_id.clone()))
                            .unwrap();
                        from_node.context.get_audio_pipe(&output).await
                    };

                    let to_node_context = {
                        let nodes_lock = self.inner.nodes.lock().await;
                        let to_node = nodes_lock
                            .get(&NodeId::new_from(connection.to_node_id.clone()))
                            .unwrap();
                        to_node.context.clone()
                    };

                    to_node_context
                        .connect_audio_pipe(&input, audio_pipe)
                        .await
                        .unwrap();

                    audio_pipe_connected(
                        PhaneronState {
                            context: self.context.clone(),
                            inner: self.inner.clone(),
                        },
                        input,
                        output,
                    )
                    .await;
                }
            }
        }

        Ok(())
    }

    async fn add_node<'a>(
        &self,
        graph_id: &'a GraphId,
        node_id: &'a NodeId,
        name: Option<String>,
        node_context: NodeRunContext,
        node: Arc<Node>,
        mut node_event_rx: tokio::sync::mpsc::UnboundedReceiver<NodeEvent>,
        semaphore_provider: ChannelSemaphoreProvider,
    ) {
        let mut graphs = self.inner.graphs.lock().await;
        let graph_entry = graphs.entry(graph_id.clone()).or_default();
        graph_entry.push(node_id.clone());

        let mut nodes = self.inner.nodes.lock().await;
        nodes.insert(
            node_id.clone(),
            PhaneronStateNode {
                name,
                context: node_context.clone(),
            },
        );

        let pending_state_channel = node_context.get_pending_state_channel();

        // Block and handle initial events
        while let Ok(event) = node_event_rx.try_recv() {
            handle_node_event(event, node_context.clone()).await;
        }

        tokio::spawn(run_node(
            self.context.clone(),
            node_context,
            node,
            pending_state_channel,
            self.get_node_event_channel().await,
            node_event_rx,
            semaphore_provider,
        ));

        self.inner.state_event_tx.send(()).ok();
    }

    pub async fn get_node_event_channel(
        &self,
    ) -> tokio::sync::mpsc::UnboundedSender<NodeStateEvent> {
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

    pub async fn get_node_state(&self, graph_id: &GraphId, node_id: &NodeId) -> Option<String> {
        self.inner.node_states.lock().await.get(node_id).cloned()
    }

    pub async fn get_available_audio_inputs(
        &self,
        graph_id: &GraphId,
        node_id: &NodeId,
    ) -> Vec<AudioInputId> {
        self.inner
            .audio_inputs
            .lock()
            .await
            .get(node_id)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn get_available_video_inputs(
        &self,
        graph_id: &GraphId,
        node_id: &NodeId,
    ) -> Vec<VideoInputId> {
        self.inner
            .video_inputs
            .lock()
            .await
            .get(node_id)
            .cloned()
            .unwrap_or_default()
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
    node_event_tx: tokio::sync::mpsc::UnboundedSender<NodeStateEvent>,
    state_event_tx: tokio::sync::broadcast::Sender<()>,
}

impl PhaneronStateInner {
    fn new(
        node_event_tx: tokio::sync::mpsc::UnboundedSender<NodeStateEvent>,
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
    context: NodeRunContext,
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
    mut node_event_rx: tokio::sync::mpsc::UnboundedReceiver<NodeStateEvent>,
    state: PhaneronState,
    state_event_tx: tokio::sync::broadcast::Sender<()>,
) {
    while let Some(event) = node_event_rx.recv().await {
        let state_modified = match event {
            NodeStateEvent::StateChanged(node_id, new_state) => {
                node_state_changed(state.clone(), node_id, new_state).await
            }
            NodeStateEvent::AudioInputAdded(node_id, audio_input_id) => {
                audio_input_added(state.clone(), node_id, audio_input_id).await
            }
            NodeStateEvent::VideoInputAdded(node_id, video_input_id) => {
                video_input_added(state.clone(), node_id, video_input_id).await
            }
            NodeStateEvent::AudioOutputAdded(node_id, audio_output_id) => {
                audio_output_added(state.clone(), node_id, audio_output_id).await
            }
            NodeStateEvent::VideoOutputAdded(node_id, video_output_id) => {
                video_output_added(state.clone(), node_id, video_output_id).await
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
