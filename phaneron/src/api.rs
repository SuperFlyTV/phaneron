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

use crate::api::rest::request::{
    GetGraphNodeInputConnectionsParams, PutGraphNodeParams, WebSocketUpgradeRequest,
};
use crate::api::rest::response::{
    AvailablePluginNode, GetAvailablePluginNodes200Response, GetGraph200ResponseNode,
    GetGraphNodeOutputs200Response, GetNodeInputConnection200Response, GetNodeState200Response,
    GraphNodeDescription, GraphNodeInput, InputNotFound404Response,
    InputTypeDoesNotMatch409Response, PhaneronGraphNode, PhaneronStateNodeAudioInput,
};
use crate::plugins::{PhaneronPluginsState, PluginId};
use crate::state::{PhaneronState, PhaneronStateRepresentation};
use crate::{CreateNode, GraphId, NodeId, PluginManager};
use abi_stable::reexports::SelfOps;
use axum::extract::ws::Message;
use axum::extract::{State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::post;
use axum::Json;
use axum::{
    body::Bytes,
    extract::Path,
    http::{header, HeaderValue, Method},
    response::IntoResponse,
    routing::get,
    Router,
};
use phaneron_plugin::{AudioInputId, AudioOutputId, VideoInputId, VideoOutputId};
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::timeout::TimeoutLayer;
use tower_http::ServiceBuilderExt;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::info;
use uuid::Uuid;

use self::message::ServerEvent;
use self::rest::request::{
    AddGraphNodeRequest, AddGraphRequest, ConnectGraphNodeInputParams,
    ConnectGraphNodeInputRequest, DisconnectGraphNodeInputParams, GetGraphNodeInputsParams,
    GetGraphNodeOutputsParams, GetGraphNodeParams, GetGraphNodeStateParams,
    GetNodeStateSchemaParams, PutGraphNodeRequest, RegisterRequest,
};
use self::rest::response::{
    AddGraph200Response, AddGraphNode200Response, AddOrUpdateGraphNode200Response,
    ConnectGrahNodeInput200Response, GetAvailablePlugins200Response, GetGraph200Response,
    GetGraphNodeInputs200Response, GetGraphNodes200Response, GetGraphs200Response,
    GetPhaneronState200Response, GraphDescription, GraphNodeOutput, GraphNotFound404Response,
    NodeNotFound404Response, NodeTypeDoesNotMatch409Response, NodeTypeNotFound404Response,
    PhaneronGraphNodeAudioInput, PhaneronGraphNodeAudioOutput, PhaneronGraphNodeVideoInput,
    PhaneronGraphNodeVideoOutput, PhaneronStateNodeAudioOutput, PhaneronStateNodeVideoInput,
    PhaneronStateNodeVideoOutput, PluginNotFound404Response, RegisterResponse,
};

mod message;
mod rest;
mod ws;

#[derive(Debug, Clone)]
pub struct Client {
    pub user_id: String,
    pub topics: Vec<String>,
    pub sender: Option<tokio::sync::broadcast::Sender<Message>>,
}

type Clients = Arc<Mutex<HashMap<Uuid, Client>>>;

pub async fn initialize_api(state_context: PhaneronState, plugins_context: &Arc<PluginManager>) {
    info!("Initializing API");

    let clients: Clients = Default::default();

    let mut state_rx = state_context.subscribe().await;
    let mut plugins_state_rx = plugins_context.subscribe_to_plugins().await;

    let state = state_rx
        .recv()
        .await
        .unwrap()
        .piped(Mutex::new)
        .piped(Arc::new);

    let state_clients = clients.clone();
    let state_loop = state.clone();
    tokio::spawn(async move {
        loop {
            // TODO: Handle case of receiver closing
            if let Ok(phaneron_state) = state_rx.recv().await {
                let mut state = state_loop.lock().await;
                *state = phaneron_state.clone();

                let clients = state_clients.lock().await;
                let state_json =
                    serde_json::to_string(&ServerEvent::PhaneronState(phaneron_state)).unwrap();
                for (_, client) in clients.iter() {
                    if let Some(sender) = &client.sender {
                        let message: Message = Message::Text(state_json.clone());
                        // TODO: Do something if this fails
                        sender.send(message).ok();
                    }
                }
            }
        }
    });

    let plugins_state = plugins_state_rx
        .recv()
        .await
        .unwrap()
        .piped(Mutex::new)
        .piped(Arc::new);

    let plugins_state_loop = plugins_state.clone();
    tokio::spawn(async move {
        loop {
            // TODO: Handle case of receiver closing
            if let Ok(plugins_state) = plugins_state_rx.recv().await {
                let mut state = plugins_state_loop.lock().await;
                *state = plugins_state;
            }
        }
    });

    let app_state = AppState {
        context: state_context,
        phaneron_state: state,
        plugins_state,
        clients,
    };

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080));
    info!("Listening on {}", addr);
    // TODO: This could fail, need to figure out how to get a result from this
    let _ = axum::Server::bind(&addr)
        .serve(app(app_state).into_make_service())
        .await;
}

#[derive(Clone)]
struct AppState {
    context: PhaneronState,
    phaneron_state: Arc<Mutex<PhaneronStateRepresentation>>, // TODO: Should be a RWLock
    plugins_state: Arc<Mutex<PhaneronPluginsState>>,         // TODO: Should be a RWLock
    clients: Clients,
}

fn app(state: AppState) -> Router {
    let sensitive_headers: Arc<[_]> = vec![header::AUTHORIZATION, header::COOKIE].into();
    let middleware = ServiceBuilder::new()
        // Mark the `Authorization` and `Cookie` headers as sensitive so it doesn't show in logs
        .sensitive_request_headers(sensitive_headers.clone())
        // Add high level tracing/logging to all requests
        .layer(
            TraceLayer::new_for_http()
                .on_body_chunk(|chunk: &Bytes, latency: Duration, _: &tracing::Span| {
                    tracing::trace!(size_bytes = chunk.len(), latency = ?latency, "sending body chunk")
                })
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_response(DefaultOnResponse::new().include_headers(true).latency_unit(LatencyUnit::Micros)),
        )
        .sensitive_response_headers(sensitive_headers)
        // Set a timeout
        .layer(TimeoutLayer::new(Duration::from_secs(10)))
        // Box the response body so it implements `Default` which is required by axum
        .map_response_body(axum::body::boxed)
        // Compress responses
        .compression()
        // Set a `Content-Type` if there isn't one already.
        .insert_response_header_if_not_present(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );

    let cors = CorsLayer::new()
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any)
        .allow_origin(Any)
        .allow_credentials(false);

    Router::new()
        .route("/", get(get_index))
        .route("/state", get(get_state))
        .route("/ws/:userId/connection", post(register_handler))
        .route(
            "/ws/:userId/connection/:clientId",
            get(state_ws).delete(unregister_handler),
        )
        .route("/plugins", get(get_plugins))
        .route("/plugins/:pluginId/nodes", get(get_plugin_nodes))
        .route(
            "/plugins/:pluginId/nodes/:nodeId/state-schema",
            get(get_node_state_schema),
        )
        .route("/graphs", get(get_graphs).post(add_graph))
        .route("/graphs/:graphId", get(get_graph))
        .route(
            "/graphs/:graphId/nodes",
            get(get_graph_nodes).post(add_graph_node),
        )
        .route(
            "/graphs/:graphId/nodes/:nodeId",
            get(get_graph_node).put(add_or_update_graph_node),
        )
        .route(
            "/graphs/:graphId/nodes/:nodeId/state",
            get(get_graph_node_state),
        )
        .route(
            "/graphs/:graphId/nodes/:nodeId/inputs",
            get(get_graph_node_inputs),
        )
        .route(
            "/graphs/:graphId/nodes/:nodeId/inputs/:inputId",
            get(get_graph_node_input_connections)
                .put(connect_graph_node_input)
                .delete(disconnect_graph_node_input),
        )
        .route(
            "/graphs/:graphId/nodes/:nodeId/outputs",
            get(get_graph_node_outputs),
        )
        .layer(middleware)
        .layer(cors)
        .with_state(state)
}

async fn get_index() -> Html<String> {
    let phaneron_version = clap::crate_version!();
    Html(format!("Phaneron {}", phaneron_version))
}

#[axum::debug_handler]
async fn get_state(state: State<AppState>) -> Json<GetPhaneronState200Response> {
    let phaneron_state = state.phaneron_state.lock().await;

    let mut nodes = HashMap::new();
    for (node_id, node) in phaneron_state.nodes.iter() {
        nodes.insert(node_id.clone(), node.clone().into());
    }

    let mut audio_inputs = HashMap::new();
    for (node_id, audio_input_ids) in phaneron_state.audio_inputs.iter() {
        audio_inputs.insert(
            node_id.clone(),
            audio_input_ids
                .iter()
                .map(|input_id| PhaneronStateNodeAudioInput {
                    id: input_id.clone(),
                })
                .collect(),
        );
    }

    let mut audio_outputs = HashMap::new();
    for (node_id, audio_output_ids) in phaneron_state.audio_outputs.iter() {
        audio_outputs.insert(
            node_id.clone(),
            audio_output_ids
                .iter()
                .map(|output_id| PhaneronStateNodeAudioOutput {
                    id: output_id.clone(),
                })
                .collect(),
        );
    }

    let mut video_inputs = HashMap::new();
    for (node_id, video_input_ids) in phaneron_state.video_inputs.iter() {
        video_inputs.insert(
            node_id.clone(),
            video_input_ids
                .iter()
                .map(|input_id| PhaneronStateNodeVideoInput {
                    id: input_id.clone(),
                })
                .collect(),
        );
    }

    let mut video_outputs = HashMap::new();
    for (node_id, video_output_ids) in phaneron_state.video_outputs.iter() {
        video_outputs.insert(
            node_id.clone(),
            video_output_ids
                .iter()
                .map(|output_id| PhaneronStateNodeVideoOutput {
                    id: output_id.clone(),
                })
                .collect(),
        );
    }

    Json(GetPhaneronState200Response {
        nodes,
        audio_inputs,
        audio_outputs,
        video_inputs,
        video_outputs,
        connections: phaneron_state.connections.clone(),
    })
}

#[axum::debug_handler]
async fn register_handler(
    state: State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Json<RegisterResponse> {
    info!("Register request: {:?}", body);
    let user_id = body.user_id;
    let uuid = Uuid::new_v4();
    info!("Creating connection with Id {}", uuid);

    register_client(uuid, user_id.clone(), state.clients.clone()).await;
    Json(RegisterResponse {
        url: format!("ws://127.0.0.1:8080/ws/{user_id}/connection/{uuid}"),
    })
}

async fn register_client(id: Uuid, user_id: String, clients: Clients) {
    clients.lock().await.insert(
        id,
        Client {
            user_id,
            topics: vec![],
            sender: None,
        },
    );
}

#[axum::debug_handler]
async fn unregister_handler(state: State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    state.clients.lock().await.remove(&id);
    StatusCode::OK
}

#[axum::debug_handler]
async fn state_ws(
    state: State<AppState>,
    ws: WebSocketUpgrade,
    Path(WebSocketUpgradeRequest {
        user_id: _,
        client_id,
    }): Path<WebSocketUpgradeRequest>,
) -> impl IntoResponse {
    info!("Connection request with Id {}", client_id);
    let client = state.clients.lock().await.get(&client_id).cloned();
    match client {
        Some(c) => Ok(ws.on_upgrade(move |socket| {
            ws::client_connection(
                state.context.clone(),
                socket,
                client_id,
                state.phaneron_state.clone(),
                state.clients.clone(),
                c,
            )
        })),
        None => Err("Client not found"),
    }
}

#[axum::debug_handler]
async fn get_plugins(state: State<AppState>) -> Json<GetAvailablePlugins200Response> {
    let plugins_state = state.plugins_state.lock().await;
    let mut plugins: Vec<rest::response::PluginDescription> = vec![];
    for (plugin_id, plugin_nodes) in plugins_state.plugins_and_node_types.iter() {
        let mut nodes = vec![];
        for node_id in plugin_nodes.iter() {
            let node = plugins_state.node_descriptions.get(node_id).unwrap();
            nodes.push(rest::response::PluginNodeDescription {
                id: node.id.clone(),
                name: node.name.clone(),
            });
        }
        plugins.push(rest::response::PluginDescription {
            id: plugin_id.to_string(),
            nodes,
        })
    }

    Json(GetAvailablePlugins200Response { plugins })
}

async fn get_plugin_nodes(
    state: State<AppState>,
    Path(plugin_id): Path<Uuid>,
) -> impl IntoResponse {
    let plugins_state = state.plugins_state.lock().await;
    let plugin = plugins_state
        .plugins_and_node_types
        .get(&PluginId::new_from(plugin_id.to_string()));

    match plugin {
        Some(plugin_nodes) => {
            let nodes: Vec<AvailablePluginNode> = plugin_nodes
                .iter()
                .map(|node_id| {
                    let node = plugins_state.node_descriptions.get(node_id).unwrap();
                    AvailablePluginNode {
                        id: node.id.clone(),
                        name: node.name.clone(),
                    }
                })
                .collect();
            (
                StatusCode::OK,
                Json(GetAvailablePluginNodes200Response { nodes }),
            )
                .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(PluginNotFound404Response::new(plugin_id.to_string())),
        )
            .into_response(),
    }
}

#[axum::debug_handler]
async fn get_node_state_schema(
    state: State<AppState>,
    Path(GetNodeStateSchemaParams { plugin_id, node_id }): Path<GetNodeStateSchemaParams>,
) -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}

#[axum::debug_handler]
async fn get_graphs(state: State<AppState>) -> Json<GetGraphs200Response> {
    let mut graphs: Vec<GraphDescription> = vec![];
    let phaneron_state = state.phaneron_state.lock().await;
    for (graph_id, graph) in phaneron_state.graphs.iter() {
        let mut nodes: Vec<GraphNodeDescription> = vec![];
        for node_id in graph.nodes.iter() {
            let node = phaneron_state.nodes.get(node_id).unwrap();
            nodes.push(GraphNodeDescription {
                id: node_id.clone(),
                name: node.name.clone(),
            });
        }

        graphs.push(GraphDescription {
            id: graph_id.clone(),
            name: graph.name.clone(),
            nodes,
        });
    }

    Json(GetGraphs200Response { graphs })
}

#[axum::debug_handler]
async fn add_graph(state: State<AppState>, Json(body): Json<AddGraphRequest>) -> impl IntoResponse {
    let graph_id = GraphId::default();
    match state.context.add_grah(&graph_id, body.name).await {
        Ok(_) => Json(AddGraph200Response {
            id: graph_id.to_string(),
        })
        .into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    }
}

#[axum::debug_handler]
async fn get_graph(state: State<AppState>, Path(graph_id): Path<String>) -> impl IntoResponse {
    let phaneron_state = state.phaneron_state.lock().await;
    let graph = phaneron_state.graphs.get(&graph_id);
    let graph = match graph {
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(GraphNotFound404Response::new(graph_id)),
            )
                .into_response();
        }
        Some(graph) => graph,
    };

    let mut nodes = Vec::with_capacity(graph.nodes.len());
    for node_id in graph.nodes.iter() {
        let node = phaneron_state.nodes.get(node_id).unwrap();
        nodes.push(GetGraph200ResponseNode {
            id: node_id.clone(),
            name: node.name.clone(),
        });
    }

    (
        StatusCode::OK,
        Json(GetGraph200Response {
            id: graph_id,
            name: graph.name.clone(),
            nodes,
        }),
    )
        .into_response()
}

#[axum::debug_handler]
async fn get_graph_nodes(
    state: State<AppState>,
    Path(graph_id): Path<String>,
) -> impl IntoResponse {
    let phaneron_state = state.phaneron_state.lock().await;

    let graph = match phaneron_state.graphs.get(&graph_id) {
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(GraphNotFound404Response::new(graph_id)),
            )
                .into_response();
        }
        Some(graph) => graph,
    };

    let mut nodes: Vec<PhaneronGraphNode> = vec![];
    for node_id in graph.nodes.iter() {
        let node = phaneron_state.nodes.get(node_id).unwrap();
        nodes.push(PhaneronGraphNode {
            id: node_id.clone(),
            name: node.name.clone(),
            node_type: node.node_type.clone(),
            state: node.state.clone(),
            audio_inputs: phaneron_state
                .audio_inputs
                .get(node_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|id| PhaneronGraphNodeAudioInput { id })
                .collect(),
            video_inputs: phaneron_state
                .video_inputs
                .get(node_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|id| PhaneronGraphNodeVideoInput { id })
                .collect(),
            audio_outputs: phaneron_state
                .audio_inputs
                .get(node_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|id| PhaneronGraphNodeAudioOutput { id })
                .collect(),
            video_outputs: phaneron_state
                .audio_inputs
                .get(node_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|id| PhaneronGraphNodeVideoOutput { id })
                .collect(),
        })
    }
    (StatusCode::OK, Json(GetGraphNodes200Response { nodes })).into_response()
}

async fn add_graph_node(
    state: State<AppState>,
    Path(graph_id): Path<Uuid>,
    Json(body): Json<AddGraphNodeRequest>,
) -> impl IntoResponse {
    let node_id = NodeId::default();
    match state
        .context
        .add_node(
            &GraphId::new_from(graph_id.to_string()),
            &node_id,
            CreateNode {
                node_type: body.node_type.clone(),
                node_name: body.name,
                state: body.state,
                configuration: body.configuration,
            },
        )
        .await
    {
        Ok(_) => Json(AddGraphNode200Response {
            id: node_id.to_string(),
        })
        .into_response(),
        Err(err) => match err {
            crate::state::AddNodeError::GraphDoesNotExist => (
                StatusCode::NOT_FOUND,
                Json(GraphNotFound404Response::new(graph_id.to_string())),
            )
                .into_response(),
            crate::state::AddNodeError::NodeTypeDoesNotExist => (
                StatusCode::NOT_FOUND,
                Json(NodeTypeNotFound404Response::new(body.node_type)),
            )
                .into_response(),
        },
    }
}

#[axum::debug_handler]
async fn get_graph_node(
    state: State<AppState>,
    Path(GetGraphNodeParams { graph_id, node_id }): Path<GetGraphNodeParams>,
) -> impl IntoResponse {
    let phaneron_state = state.phaneron_state.lock().await;
    let graph = phaneron_state.graphs.get(&graph_id);
    if graph.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(GraphNotFound404Response::new(graph_id)),
        )
            .into_response();
    }

    let node = phaneron_state.nodes.get(&node_id);
    let node = match node {
        Some(node) => node,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(NodeNotFound404Response::new(node_id)),
            )
                .into_response()
        }
    };

    Json(PhaneronGraphNode {
        id: node_id.clone(),
        name: node.name.clone(),
        node_type: node.node_type.clone(),
        state: node.state.clone(),
        audio_inputs: phaneron_state
            .audio_inputs
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|input_id| PhaneronGraphNodeAudioInput {
                id: input_id.clone(),
            })
            .collect(),
        audio_outputs: phaneron_state
            .audio_outputs
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|input_id| PhaneronGraphNodeAudioOutput {
                id: input_id.clone(),
            })
            .collect(),
        video_inputs: phaneron_state
            .video_inputs
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|input_id| PhaneronGraphNodeVideoInput {
                id: input_id.clone(),
            })
            .collect(),
        video_outputs: phaneron_state
            .video_outputs
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|input_id| PhaneronGraphNodeVideoOutput {
                id: input_id.clone(),
            })
            .collect(),
    })
    .into_response()
}

#[axum::debug_handler]
async fn add_or_update_graph_node(
    state: State<AppState>,
    Path(PutGraphNodeParams { graph_id, node_id }): Path<PutGraphNodeParams>,
    Json(body): Json<PutGraphNodeRequest>,
) -> impl IntoResponse {
    let phaneron_state = state.phaneron_state.lock().await;
    if !phaneron_state.graphs.contains_key(&graph_id.to_string()) {
        return (
            StatusCode::NOT_FOUND,
            Json(GraphNotFound404Response::new(graph_id)),
        )
            .into_response();
    }

    let node = phaneron_state.nodes.get(&node_id);
    match node {
        Some(existing_node) => {
            if existing_node.node_type != body.node_type {
                return (
                    StatusCode::CONFLICT,
                    Json(NodeTypeDoesNotMatch409Response {
                        existing_node_type: existing_node.node_type.clone(),
                    }),
                )
                    .into_response();
            }

            let graph_id = GraphId::new_from(graph_id);
            let node_id = NodeId::new_from(node_id);
            state
                .context
                .set_node_name(&graph_id, &node_id, body.name)
                .await;
            state
                .context
                .set_node_state(&graph_id, &node_id, body.state)
                .await;
            // TODO: Set configuration

            (
                StatusCode::OK,
                Json(AddOrUpdateGraphNode200Response {
                    id: node_id.to_string(),
                }),
            )
                .into_response()
        }
        None => {
            let node_id = NodeId::new_from(node_id);
            match state
                .context
                .add_node(
                    &GraphId::new_from(graph_id.to_string()),
                    &node_id,
                    CreateNode {
                        node_type: body.node_type.clone(),
                        node_name: body.name,
                        state: body.state,
                        configuration: body.configuration,
                    },
                )
                .await
            {
                Ok(_) => Json(AddGraphNode200Response {
                    id: node_id.to_string(),
                })
                .into_response(),
                Err(err) => match err {
                    crate::state::AddNodeError::GraphDoesNotExist => (
                        StatusCode::NOT_FOUND,
                        Json(GraphNotFound404Response::new(graph_id)),
                    )
                        .into_response(),
                    crate::state::AddNodeError::NodeTypeDoesNotExist => (
                        StatusCode::NOT_FOUND,
                        Json(NodeTypeNotFound404Response::new(body.node_type)),
                    )
                        .into_response(),
                },
            }
        }
    }
}

#[axum::debug_handler]
async fn get_graph_node_state(
    state: State<AppState>,
    Path(GetGraphNodeStateParams { graph_id, node_id }): Path<GetGraphNodeStateParams>,
) -> impl IntoResponse {
    let phaneron_state = state.phaneron_state.lock().await;
    let graph = phaneron_state.graphs.get(&graph_id);
    if graph.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(GraphNotFound404Response::new(graph_id)),
        )
            .into_response();
    }

    let node = phaneron_state.nodes.get(&node_id);
    let node = match node {
        Some(node) => node,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(NodeNotFound404Response::new(node_id)),
            )
                .into_response()
        }
    };

    Json(GetNodeState200Response {
        state: node.state.clone(),
    })
    .into_response()
}

#[axum::debug_handler]
async fn get_graph_node_inputs(
    state: State<AppState>,
    Path(GetGraphNodeInputsParams { graph_id, node_id }): Path<GetGraphNodeInputsParams>,
) -> impl IntoResponse {
    let phaneron_state = state.phaneron_state.lock().await;
    let graph = phaneron_state.graphs.get(&graph_id);
    if graph.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(GraphNotFound404Response::new(graph_id)),
        )
            .into_response();
    }

    let node = phaneron_state.nodes.get(&node_id);
    match node {
        Some(node) => node,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(NodeNotFound404Response::new(node_id)),
            )
                .into_response()
        }
    };

    let audio_inputs = phaneron_state
        .audio_inputs
        .get(&node_id)
        .cloned()
        .unwrap_or_default();

    let audio_inputs: Vec<GraphNodeInput> = audio_inputs
        .iter()
        .map(|input_id| GraphNodeInput {
            id: input_id.clone(),
            input_type: "audio".to_string(),
            connected_output_id: phaneron_state.connections.get(input_id).cloned(),
        })
        .collect();

    let video_inputs = phaneron_state
        .video_inputs
        .get(&node_id)
        .cloned()
        .unwrap_or_default();

    let video_inputs: Vec<GraphNodeInput> = video_inputs
        .iter()
        .map(|input_id| GraphNodeInput {
            id: input_id.clone(),
            input_type: "video".to_string(),
            connected_output_id: phaneron_state.connections.get(input_id).cloned(),
        })
        .collect();

    Json(GetGraphNodeInputs200Response {
        inputs: vec![audio_inputs, video_inputs].concat(),
    })
    .into_response()
}

#[axum::debug_handler]
async fn get_graph_node_input_connections(
    state: State<AppState>,
    Path(GetGraphNodeInputConnectionsParams {
        graph_id,
        node_id,
        input_id,
    }): Path<GetGraphNodeInputConnectionsParams>,
) -> impl IntoResponse {
    let phaneron_state = state.phaneron_state.lock().await;
    if !phaneron_state.graphs.contains_key(&graph_id.to_string()) {
        return (
            StatusCode::NOT_FOUND,
            Json(GraphNotFound404Response::new(graph_id)),
        )
            .into_response();
    }

    if !phaneron_state.nodes.contains_key(&node_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(NodeNotFound404Response::new(node_id)),
        )
            .into_response();
    };

    let audio_inputs = phaneron_state.audio_inputs.get(&node_id).unwrap();
    let video_inputs = phaneron_state.video_inputs.get(&node_id).unwrap();

    let input_type = if audio_inputs.contains(&input_id) {
        "audio".to_string()
    } else if video_inputs.contains(&input_id) {
        "video".to_string()
    } else {
        return (
            StatusCode::NOT_FOUND,
            Json(InputNotFound404Response::new(input_id)),
        )
            .into_response();
    };

    match phaneron_state.connections.get(&input_id) {
        Some(connection) => (
            StatusCode::OK,
            Json(GetNodeInputConnection200Response {
                id: input_id.clone(),
                input_type,
                connected_output_id: Some(connection.clone()),
            }),
        )
            .into_response(),
        None => (
            StatusCode::OK,
            Json(GetNodeInputConnection200Response {
                id: input_id.clone(),
                input_type,
                connected_output_id: None,
            }),
        )
            .into_response(),
    };

    todo!()
}

#[axum::debug_handler]
async fn connect_graph_node_input(
    state: State<AppState>,
    Path(ConnectGraphNodeInputParams {
        graph_id,
        node_id,
        input_id,
    }): Path<ConnectGraphNodeInputParams>,
    Json(body): Json<ConnectGraphNodeInputRequest>,
) -> impl IntoResponse {
    let phaneron_state = state.phaneron_state.lock().await;
    if !phaneron_state.graphs.contains_key(&graph_id.to_string()) {
        return (
            StatusCode::NOT_FOUND,
            Json(GraphNotFound404Response::new(graph_id)),
        )
            .into_response();
    }

    if !phaneron_state.nodes.contains_key(&node_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(NodeNotFound404Response::new(node_id)),
        )
            .into_response();
    };

    if !phaneron_state
        .nodes
        .contains_key(&body.connect_from_node_id)
    {
        return (
            StatusCode::NOT_FOUND,
            Json(NodeNotFound404Response::new(body.connect_from_node_id)),
        )
            .into_response();
    };

    let audio_inputs = phaneron_state.audio_inputs.get(&node_id).unwrap();
    let video_inputs = phaneron_state.video_inputs.get(&node_id).unwrap();

    let input_is_audio = if audio_inputs.contains(&input_id) {
        true
    } else if video_inputs.contains(&input_id) {
        false
    } else {
        return (
            StatusCode::NOT_FOUND,
            Json(InputNotFound404Response::new(input_id)),
        )
            .into_response();
    };

    let from_node_outputs = if input_is_audio {
        phaneron_state
            .audio_outputs
            .get(&body.connect_from_node_id)
            .cloned()
            .unwrap_or_default()
    } else {
        phaneron_state
            .video_outputs
            .get(&body.connect_from_node_id)
            .cloned()
            .unwrap_or_default()
    };

    if !from_node_outputs.contains(&body.connect_from_output_id) {
        let input_type = if input_is_audio {
            "audio".to_string()
        } else {
            "video".to_string()
        };
        return (
            StatusCode::NOT_FOUND,
            Json(InputTypeDoesNotMatch409Response { input_type }),
        )
            .into_response();
    }

    let result = if input_is_audio {
        state
            .context
            .make_audio_connection(
                &NodeId::new_from(body.connect_from_node_id),
                &AudioOutputId::new_from(body.connect_from_output_id.clone().into()),
                &NodeId::new_from(node_id),
                &AudioInputId::new_from(input_id.clone().into()),
            )
            .await
    } else {
        state
            .context
            .make_video_connection(
                &NodeId::new_from(body.connect_from_node_id),
                &VideoOutputId::new_from(body.connect_from_output_id.clone().into()),
                &NodeId::new_from(node_id),
                &VideoInputId::new_from(input_id.clone().into()),
            )
            .await
    };

    if result.is_ok() {
        let input_type = if input_is_audio {
            "audio".to_string()
        } else {
            "video".to_string()
        };
        (
            StatusCode::OK,
            Json(ConnectGrahNodeInput200Response {
                id: input_id.clone(),
                input_type,
                connected_output_id: Some(body.connect_from_output_id),
            }),
        )
            .into_response()
    } else {
        todo!("Handle error from making connections")
    }
}

#[axum::debug_handler]
async fn disconnect_graph_node_input(
    state: State<AppState>,
    Path(DisconnectGraphNodeInputParams {
        graph_id,
        node_id,
        input_id,
    }): Path<DisconnectGraphNodeInputParams>,
) -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}

#[axum::debug_handler]
async fn get_graph_node_outputs(
    state: State<AppState>,
    Path(GetGraphNodeOutputsParams { graph_id, node_id }): Path<GetGraphNodeOutputsParams>,
) -> impl IntoResponse {
    let phaneron_state = state.phaneron_state.lock().await;
    if !phaneron_state.graphs.contains_key(&graph_id.to_string()) {
        return (
            StatusCode::NOT_FOUND,
            Json(GraphNotFound404Response::new(graph_id)),
        )
            .into_response();
    }

    if !phaneron_state.nodes.contains_key(&node_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(NodeNotFound404Response::new(node_id)),
        )
            .into_response();
    };

    let audio_inputs = phaneron_state
        .audio_inputs
        .get(&node_id)
        .cloned()
        .unwrap_or_default();

    let audio_inputs: Vec<GraphNodeOutput> = audio_inputs
        .iter()
        .map(|input_id| GraphNodeOutput {
            id: input_id.clone(),
            output_type: "audio".to_string(),
            connected_input_ids: phaneron_state
                .output_to_input_connections
                .get(input_id)
                .cloned()
                .unwrap_or_default(),
        })
        .collect();

    let video_inputs = phaneron_state
        .video_inputs
        .get(&node_id)
        .cloned()
        .unwrap_or_default();

    let video_inputs: Vec<GraphNodeOutput> = video_inputs
        .iter()
        .map(|input_id| GraphNodeOutput {
            id: input_id.clone(),
            output_type: "video".to_string(),
            connected_input_ids: phaneron_state
                .output_to_input_connections
                .get(input_id)
                .cloned()
                .unwrap_or_default(),
        })
        .collect();

    Json(GetGraphNodeOutputs200Response {
        outputs: vec![audio_inputs, video_inputs].concat(),
    })
    .into_response()
}
