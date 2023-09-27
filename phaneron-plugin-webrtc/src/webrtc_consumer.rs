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

use std::net::Ipv4Addr;
use std::sync::Mutex;
use std::time::SystemTime;
use std::{net::SocketAddr, sync::Arc, time::Duration};

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{ROption, RString};
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::Method;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Json;
use axum::{
    http::{header, HeaderValue, StatusCode},
    Router,
};
use byteorder::{ByteOrder, LittleEndian};
use phaneron_plugin::types::{FromAudioF32, FromRGBA, NodeContext};
use phaneron_plugin::{
    traits::Node_TO, types::Node, types::ProcessFrameContext, AudioChannelLayout, AudioFormat,
    AudioInputId, ColourSpace, InterlaceMode, VideoFormat, VideoInputId,
};
use tokio::time::{Instant, MissedTickBehavior};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tower_http::{LatencyUnit, ServiceBuilderExt};
use tracing::{debug, info};
use webrtc::api::media_engine::{MIME_TYPE_OPUS, MIME_TYPE_VP8};
use webrtc::{
    api::{
        interceptor_registry::register_default_interceptors, media_engine::MediaEngine, APIBuilder,
    },
    ice_transport::ice_server::RTCIceServer,
    interceptor::registry::Registry,
    media::Sample,
    peer_connection::{
        configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription, RTCPeerConnection,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::{track_local_static_sample::TrackLocalStaticSample, TrackLocal},
};

lazy_static! {
    static ref PEER_CONNECTION_MUTEX: Arc<Mutex<Option<Arc<RTCPeerConnection>>>> =
        Arc::new(Mutex::new(None));
}

pub struct WebRTCConsumerHandle {
    node_id: String,
}
impl WebRTCConsumerHandle {
    pub(super) fn new(node_id: String) -> Self {
        Self { node_id }
    }
}
impl phaneron_plugin::traits::NodeHandle for WebRTCConsumerHandle {
    fn initialize(&self, context: NodeContext, _configuration: ROption<RString>) -> Node {
        let node = WebRTCConsumer::new(self.node_id.clone(), context);

        Node_TO::from_value(node, TD_Opaque)
    }
}

pub struct WebRTCConsumer {
    node_id: String,
    context: NodeContext,
    start: Mutex<Option<tokio::time::Instant>>,
    interval: Mutex<Option<tokio::time::Interval>>,
    from_rgba: Mutex<Option<FromRGBA>>,
    from_audio_f32: Mutex<Option<FromAudioF32>>,
    vpx: Mutex<Option<VPXEncoder>>,
    opus: Mutex<Option<opus::Encoder>>,
    video_tracks: VideoTracks,
    audio_tracks: AudioTracks,
    tokio_handle: tokio::runtime::Handle,
    tokio_terminate_sender: tokio::sync::oneshot::Sender<()>,
    video_input: VideoInputId,
    audio_input: AudioInputId,
}

impl WebRTCConsumer {
    pub fn new(node_id: String, context: NodeContext) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let (terminate_sender, terminate_receiver) = tokio::sync::oneshot::channel::<()>();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let handle = runtime.handle();
            sender.send(handle.clone()).unwrap();
            runtime.block_on(terminate_receiver).ok();
        });

        let handle = receiver.recv().unwrap();

        // Create a MediaEngine object to configure the supported codec
        let mut m = MediaEngine::default();
        m.register_default_codecs().unwrap();

        // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
        // This provides NACKs, RTCP Reports and other features. If you use `webrtc.NewPeerConnection`
        // this is enabled by default. If you are manually managing You MUST create a InterceptorRegistry
        // for each PeerConnection.
        let mut registry = Registry::new();

        // Use the default set of Interceptors
        registry = register_default_interceptors(registry, &mut m).unwrap();

        // Create the API object with the MediaEngine
        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        // Prepare the configuration
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // Create a new RTCPeerConnection
        let peer_connection = handle.block_on(api.new_peer_connection(config)).unwrap();
        let peer_connection = Arc::new(peer_connection);
        let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

        // Set the handler for Peer connection state
        // This will notify you when the peer has connected/disconnected
        peer_connection.on_peer_connection_state_change(Box::new(
            move |s: RTCPeerConnectionState| {
                info!("Peer Connection State has changed: {s}");

                if s == RTCPeerConnectionState::Failed {
                    // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
                    // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
                    // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
                    info!("Peer Connection has gone to failed exiting");
                    let _ = done_tx.try_send(());
                }

                Box::pin(async {})
            },
        ));

        {
            let mut pcm = PEER_CONNECTION_MUTEX.lock().unwrap();
            *pcm = Some(Arc::clone(&peer_connection));
        }

        let video_tracks: VideoTracks = Default::default();
        let audio_tracks: AudioTracks = Default::default();

        let pc = {
            let pcm = PEER_CONNECTION_MUTEX.lock().unwrap();
            pcm.clone().unwrap()
        };

        let state = AppState {
            video_tracks: video_tracks.clone(),
            audio_tracks: audio_tracks.clone(),
            peer_connection: pc,
        };

        handle.spawn(serve_web_server(state));

        let video_input = context.add_video_input();

        let audio_input = context.add_audio_input();

        Self {
            node_id,
            context,
            start: Default::default(),
            interval: Default::default(),
            from_rgba: Default::default(),
            from_audio_f32: Default::default(),
            vpx: Default::default(),
            opus: Default::default(),
            video_tracks,
            audio_tracks,
            tokio_handle: handle,
            tokio_terminate_sender: terminate_sender,
            video_input,
            audio_input,
        }
    }
}

impl phaneron_plugin::traits::Node for WebRTCConsumer {
    fn apply_state(&self, state: RString) -> bool {
        false
    }
    fn process_frame(&self, frame_context: ProcessFrameContext) {
        let mut interval_lock = self.interval.lock().unwrap();
        let interval = interval_lock.get_or_insert_with(|| {
            self.tokio_handle.block_on(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(40));
                interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
                interval
            })
        });

        let mut from_rgba_lock = self.from_rgba.lock().unwrap();
        let from_rgba = from_rgba_lock.get_or_insert_with(|| {
            self.context.create_from_rgba(
                &VideoFormat::YUV420p,
                &ColourSpace::sRGB.colour_spec(),
                1920,
                1080,
                InterlaceMode::Progressive,
            )
        });
        let mut from_audio_f32_lock = self.from_audio_f32.lock().unwrap();
        let from_audio_f32 = from_audio_f32_lock.get_or_insert_with(|| {
            self.context
                .create_from_audio_f32(AudioFormat::I16, AudioChannelLayout::Mono)
        });
        let mut start_lock = self.start.lock().unwrap();
        let start = start_lock.get_or_insert(Instant::now());

        let video_frame = frame_context
            .get_video_input(&self.video_input)
            .unwrap_or(frame_context.get_black_frame())
            .clone();
        let video_frame = from_rgba.process_frame(&frame_context, video_frame.frame);

        let audio_frame = frame_context
            .get_audio_input(&self.audio_input)
            .unwrap_or(frame_context.get_silence_frame())
            .clone();

        let mut audio_encoder_lock = self.opus.lock().unwrap();
        let audio_encoder = audio_encoder_lock.get_or_insert_with(|| {
            opus::Encoder::new(48000, opus::Channels::Mono, opus::Application::Audio).unwrap()
        });
        let audio_frame = from_audio_f32.process_frame(&frame_context, audio_frame.frame);

        let copy_context = frame_context.submit().unwrap();

        let video_frame = from_rgba.copy_frame(&copy_context, video_frame);
        let video_frame: Vec<u8> = video_frame.iter().flatten().cloned().collect();

        let now = Instant::now();
        let time = now - *start;
        let ms = time.as_secs() * 1000 + time.subsec_millis() as u64;
        let video_frames = {
            let mut vpx_lock = self.vpx.lock().unwrap();
            let vpx = vpx_lock.get_or_insert_with(|| {
                let vpx = vpx_encode::Encoder::new(vpx_encode::Config {
                    width: 1920,
                    height: 1080,
                    timebase: [1, 1000],
                    bitrate: 5000,
                    codec: vpx_encode::VideoCodecId::VP8,
                })
                .unwrap();
                VPXEncoder::new(vpx)
            });
            let packets = vpx.encode(ms as i64, &video_frame).unwrap();
            packets
                .into_iter()
                .map(|frame| frame.data.to_vec())
                .collect::<Vec<Vec<u8>>>()
        };

        let audio_frame = {
            let fr = from_audio_f32.copy_frame(&copy_context, audio_frame);

            let mut frame: Vec<i16> = vec![0i16; fr.len() / 2];
            LittleEndian::read_i16_into(&fr, &mut frame);

            const OUT_SIZE: usize = 256; // TODO: This is only correct for mono up to 48kHz
            let mut out = vec![0u8; OUT_SIZE];
            let bytes = audio_encoder.encode(&frame, &mut out).unwrap();

            out[0..bytes].to_vec()
        };

        self.tokio_handle
            .block_on(async move { interval.tick().await });

        for track in self.audio_tracks.lock().unwrap().iter() {
            self.tokio_handle.spawn(write_audio_to_track(
                track.clone(),
                audio_frame.clone().into(),
            ));
        }

        for frame in video_frames {
            for track in self.video_tracks.lock().unwrap().iter() {
                self.tokio_handle
                    .spawn(write_video_to_track(track.clone(), frame.clone().into()));
            }
        }
    }
}

async fn write_video_to_track<'a>(t: Arc<TrackLocalStaticSample>, data: Bytes) {
    t.write_sample(&Sample {
        data,
        duration: Duration::from_millis(40),
        timestamp: SystemTime::now(),
        ..Default::default()
    })
    .await
    .unwrap();
}

async fn write_audio_to_track<'a>(t: Arc<TrackLocalStaticSample>, data: Bytes) {
    t.write_sample(&Sample {
        data,
        duration: Duration::from_millis(40),
        timestamp: SystemTime::now(),
        ..Default::default()
    })
    .await
    .unwrap();
}

#[derive(Clone)]
struct AppState {
    video_tracks: VideoTracks,
    audio_tracks: AudioTracks,
    peer_connection: Arc<RTCPeerConnection>,
}

async fn serve_web_server(state: AppState) {
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 9091));
    info!("Listening on {}", addr);
    let _ = axum::Server::bind(&addr)
        .serve(app(state).into_make_service())
        .await;
}

// TODO: Really we should be registering routes with Phaneron's API when the plugin loads
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

    // Build route service
    Router::new()
        .route("/createPeerConnection", post(create_peer_connection))
        .route("/addMedia", post(add_media))
        .layer(middleware)
        .layer(cors)
        .with_state(state)
}

async fn create_peer_connection(
    state: State<AppState>,
    Json(body): Json<RTCSessionDescription>,
) -> impl IntoResponse {
    if state.peer_connection.connection_state() != RTCPeerConnectionState::New {
        panic!(
            "create_peer_connection called in non-new state ({})",
            state.peer_connection.connection_state()
        );
    }

    info!("PeerConnection has been created");
    do_signaling(&state.peer_connection, body).await
}

// do_signaling exchanges all state of the local PeerConnection and is called
// every time a video is added or removed
async fn do_signaling(pc: &Arc<RTCPeerConnection>, body: RTCSessionDescription) -> Response<Body> {
    let offer = body;

    if let Err(err) = pc.set_remote_description(offer).await {
        panic!("{}", err);
    }

    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = pc.gathering_complete_promise().await;

    // Create an answer
    let answer = match pc.create_answer(None).await {
        Ok(answer) => answer,
        Err(err) => panic!("{}", err),
    };

    // Sets the LocalDescription, and starts our UDP listeners
    if let Err(err) = pc.set_local_description(answer).await {
        panic!("{}", err);
    }

    // Block until ICE Gathering is complete, disabling trickle ICE
    // we do this because we only can exchange one signaling message
    // in a production application you should exchange ICE Candidates via OnICECandidate
    let _ = gather_complete.recv().await;

    let payload = if let Some(local_desc) = pc.local_description().await {
        match serde_json::to_string(&local_desc) {
            Ok(p) => p,
            Err(err) => panic!("{}", err),
        }
    } else {
        panic!("generate local_description failed!");
    };

    let mut response = match Response::builder()
        .header("content-type", "application/json")
        .body(Body::from(payload))
    {
        Ok(res) => res,
        Err(err) => panic!("{}", err),
    };

    *response.status_mut() = StatusCode::OK;
    response
}

async fn add_media(
    state: State<AppState>,
    Json(body): Json<RTCSessionDescription>,
) -> impl IntoResponse {
    let video_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_VP8.to_owned(),
            ..Default::default()
        },
        format!("video-{}", uuid::Uuid::new_v4()),
        format!("video-{}", uuid::Uuid::new_v4()),
    ));

    let rtp_sender = match state
        .peer_connection
        .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await
    {
        Ok(rtp_sender) => rtp_sender,
        Err(err) => panic!("{}", err),
    };

    {
        let mut video_tracks = state.video_tracks.lock().unwrap();
        video_tracks.push(video_track);
    }

    // Read incoming RTCP packets
    // Before these packets are returned they are processed by interceptors. For things
    // like NACK this needs to be called.
    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
    });

    debug!("Video track has been added");

    let audio_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_OPUS.to_owned(),
            channels: 1,       // TODO
            clock_rate: 48000, // TODO
            ..Default::default()
        },
        format!("audio-{}", uuid::Uuid::new_v4()),
        format!("audio-{}", uuid::Uuid::new_v4()),
    ));

    let rtp_sender = match state
        .peer_connection
        .add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await
    {
        Ok(rtp_sender) => rtp_sender,
        Err(err) => panic!("{}", err),
    };

    {
        let mut audio_tracks = state.audio_tracks.lock().unwrap();
        audio_tracks.push(audio_track);
    }

    // Read incoming RTCP packets
    // Before these packets are returned they are processed by interceptors. For things
    // like NACK this needs to be called.
    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
    });

    debug!("Audio track has been added");

    do_signaling(&state.peer_connection, body).await
}

/// Lies to rust because we want the encoder to go into a tokio task
struct VPXEncoder {
    encoder: vpx_encode::Encoder,
}

impl VPXEncoder {
    fn new(encoder: vpx_encode::Encoder) -> Self {
        Self { encoder }
    }

    fn encode(&mut self, pts: i64, data: &[u8]) -> vpx_encode::Result<vpx_encode::Packets> {
        self.encoder.encode(pts, data)
    }
}

unsafe impl Send for VPXEncoder {}
unsafe impl Sync for VPXEncoder {}

type VideoTracks = Arc<Mutex<Vec<Arc<TrackLocalStaticSample>>>>;
type AudioTracks = Arc<Mutex<Vec<Arc<TrackLocalStaticSample>>>>;
