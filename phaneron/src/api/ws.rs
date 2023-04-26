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

use std::{future, sync::Arc};

use axum::extract::ws::{Message, WebSocket};
use futures::StreamExt;
use tokio::sync::Mutex;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, error, log::info};
use uuid::Uuid;

use crate::{
    api::message::ServerEvent,
    state::{PhaneronState, PhaneronStateRepresentation},
    GraphId, NodeId,
};

use super::{Client, Clients};

pub async fn client_connection(
    state_context: PhaneronState,
    ws: WebSocket,
    id: Uuid,
    state: Arc<Mutex<PhaneronStateRepresentation>>,
    clients: Clients,
    mut client: Client,
) {
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = tokio::sync::broadcast::channel::<Message>(10);
    let client_rcv = BroadcastStream::new(client_rcv);

    tokio::task::spawn(
        client_rcv
            .filter(|msg| future::ready(msg.is_ok()))
            .map(|msg| Ok(msg.unwrap()))
            .forward(client_ws_sender),
    );

    let phaneron_state = state.lock().await.clone();
    let state_json = serde_json::to_string(&ServerEvent::PhaneronState(phaneron_state)).unwrap();
    client_sender.send(Message::Text(state_json.clone())).ok();

    client.sender = Some(client_sender);
    clients.lock().await.insert(id, client);

    info!("{} connected", id);

    while let Some(result) = client_ws_rcv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                error!("error resolving ws message for id: {}: {}", id.clone(), e);
                break;
            }
        };
        client_msg(state_context.clone(), &id, msg, &clients).await;
    }

    clients.lock().await.remove(&id);
    info!("{} disconnected", id);
}

async fn client_msg(state_context: PhaneronState, id: &Uuid, msg: Message, clients: &Clients) {
    debug!("received message from {}: {:?}", id, msg);
    let message = match msg.into_text() {
        Ok(v) => v,
        Err(err) => {
            error!("error: {:?}", err);
            return;
        }
    };

    if message == "ping" || message == "ping\n" {
        return;
    }

    let topics_req: super::message::ClientEvent = match serde_json::from_str(&message) {
        Ok(v) => v,
        Err(e) => {
            error!("error while parsing message to topics request: {}", e);
            return;
        }
    };

    match topics_req {
        super::message::ClientEvent::Topics(topics_req) => {
            debug!("Topics req: {:?}", topics_req);
            let mut locked = clients.lock().await;
            if let Some(v) = locked.get_mut(id) {
                v.topics = topics_req.topics.clone();
                if let Some(sender) = &v.sender {
                    sender
                        .send(Message::Text(format!(
                            "You are now subscribed to {}",
                            topics_req.topics.join(",")
                        )))
                        .unwrap();
                }
            };
        }
        super::message::ClientEvent::NodeState(state) => {
            debug!("NodeState req: {:?}", state);
            state_context
                .set_node_state(
                    &GraphId::new_from("graph1".to_string()), // TODO: Remove hard-coded value
                    &NodeId::new_from(state.node_id),
                    state.state,
                )
                .await;
        }
    }
}
