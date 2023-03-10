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
use tracing::log::info;
use uuid::Uuid;

use crate::{
    api::message::RegisterResponse,
    state::{PhaneronState, PhaneronStateRepresentation},
};

use self::message::{RegisterRequest, ServerEvent};

mod message;
mod ws;

#[derive(Debug, Clone)]
pub struct Client {
    pub user_id: String,
    pub topics: Vec<String>,
    pub sender: Option<tokio::sync::broadcast::Sender<Message>>,
}

type Clients = Arc<Mutex<HashMap<Uuid, Client>>>;

pub async fn initialize_api(state_context: PhaneronState) {
    info!("Initializing API");

    let clients: Clients = Default::default();

    let mut state_rx = state_context.subscribe().await;

    let state = state_rx.recv().await.unwrap();
    let state = Arc::new(Mutex::new(state));

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

    let app_state = AppState {
        context: state_context.clone(),
        phaneron_state: state.clone(),
        clients: clients.clone(),
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
    phaneron_state: Arc<Mutex<PhaneronStateRepresentation>>,
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
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any)
        .allow_origin(Any)
        .allow_credentials(false);

    Router::new()
        .route("/", get(get_index))
        .route(
            "/register",
            post(register_handler).delete(unregister_handler),
        )
        .route("/ws/:clientId", get(state_ws))
        .layer(middleware)
        .layer(cors)
        .with_state(state)
}

async fn get_index() -> impl IntoResponse {
    let phaneron_version = clap::crate_version!();
    Html(format!("Phaneron {}", phaneron_version))
}

#[axum::debug_handler]
async fn register_handler(
    state: State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> impl IntoResponse {
    info!("Register request: {:?}", body);
    let user_id = body.user_id;
    let uuid = Uuid::new_v4();
    info!("Creating connection with Id {}", uuid);

    register_client(uuid, user_id, state.clients.clone()).await;
    Json(RegisterResponse {
        url: format!("ws://127.0.0.1:8080/ws/{uuid}"),
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
async fn unregister_handler(Path(id): Path<Uuid>, state: State<AppState>) -> impl IntoResponse {
    state.clients.lock().await.remove(&id);
    StatusCode::OK
}

async fn state_ws(
    ws: WebSocketUpgrade,
    Path(id): Path<Uuid>,
    state: State<AppState>,
) -> impl IntoResponse {
    info!("Connection request with Id {}", id);
    let client = state.clients.lock().await.get(&id).cloned();
    match client {
        Some(c) => Ok(ws.on_upgrade(move |socket| {
            ws::client_connection(
                state.context.clone(),
                socket,
                id,
                state.phaneron_state.clone(),
                state.clients.clone(),
                c,
            )
        })),
        None => Err("Client not found"),
    }
}
