use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::{net::SocketAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
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
use tokio::time::MissedTickBehavior;
use tokio::{sync::Mutex, time::Instant};
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

use crate::compute::audio_frame::AudioFrame;
use crate::compute::video_frame::VideoFrame;
use crate::format::VideoFormat;
use crate::graph::{AudioInputId, AudioOutputId, VideoInputId, VideoOutputId};
use crate::io::{FromRGBA, InterlaceMode};
use crate::node_context::{Node, ProcessFrameContext};
use crate::{colour::ColourSpace, graph::NodeId, node_context::NodeContext};

lazy_static! {
    static ref PEER_CONNECTION_MUTEX: Arc<Mutex<Option<Arc<RTCPeerConnection>>>> =
        Arc::new(Mutex::new(None));
}

pub struct WebRTCConsumerPlugin {}

impl WebRTCConsumerPlugin {
    pub fn load() -> Self {
        Self {}
    }

    pub async fn initialize(&mut self) {
        info!("WebRTC Consumer plugin initializing");
        info!("WebRTC Consumer plugin initialized");
    }

    pub async fn create_node(&mut self, node_id: NodeId) -> WebRTCConsumer {
        WebRTCConsumer::new(node_id)
    }
}

pub struct WebRTCConsumer {
    node_id: NodeId,
    context: Mutex<Option<NodeContext>>,
    start: Mutex<Option<tokio::time::Instant>>,
    interval: Mutex<Option<tokio::time::Interval>>,
    from_rgba: Mutex<Option<FromRGBA>>,
    vpx: Mutex<Option<VPXEncoder>>,
    opus: Mutex<Option<opus::Encoder>>,
    video_tracks: Mutex<Option<VideoTracks>>,
    audio_tracks: Mutex<Option<AudioTracks>>,
}

impl WebRTCConsumer {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            context: Default::default(),
            start: Default::default(),
            interval: Default::default(),
            from_rgba: Default::default(),
            vpx: Default::default(),
            opus: Default::default(),
            video_tracks: Default::default(),
            audio_tracks: Default::default(),
        }
    }

    pub async fn initialize(&mut self, context: NodeContext) {
        info!("WebRTC Consumer {} initializing", self.node_id);

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
        let peer_connection = Arc::new(api.new_peer_connection(config).await.unwrap());
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
            let mut pcm = PEER_CONNECTION_MUTEX.lock().await;
            *pcm = Some(Arc::clone(&peer_connection));
        }

        let video_tracks: VideoTracks = Default::default();
        let audio_tracks: AudioTracks = Default::default();

        let pc = {
            let pcm = PEER_CONNECTION_MUTEX.lock().await;
            pcm.clone().unwrap()
        };

        let state = AppState {
            video_tracks: video_tracks.clone(),
            audio_tracks: audio_tracks.clone(),
            peer_connection: pc,
        };

        tokio::spawn(serve_web_server(state));

        context
            .add_video_input(VideoInputId::new_from(
                "webrtc_consumer_video_input".to_string(),
            ))
            .await
            .unwrap();

        context
            .add_audio_input(AudioInputId::new_from(
                "webrtc_consumer_audio_input".to_string(),
            ))
            .await
            .unwrap();

        self.context.lock().await.replace(context);
        self.video_tracks.lock().await.replace(video_tracks);
        self.audio_tracks.lock().await.replace(audio_tracks);

        info!("WebRTC Consumer {} initialized", self.node_id);
    }
}

#[async_trait]
impl Node for WebRTCConsumer {
    async fn apply_state(&self, state: String) -> bool {
        false
    }
    async fn process_frame(
        &self,
        frame_context: ProcessFrameContext,
        video_frames: HashMap<VideoInputId, (VideoOutputId, VideoFrame)>,
        audio_frames: HashMap<AudioInputId, (AudioOutputId, AudioFrame)>,
        black_frame: (VideoOutputId, VideoFrame),
        silence_frame: (AudioOutputId, AudioFrame),
    ) {
        let context = self.context.lock().await;
        let mut interval_lock = self.interval.lock().await;
        let interval = interval_lock.get_or_insert_with(|| {
            let mut interval = tokio::time::interval(Duration::from_millis(40));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            interval
        });

        let mut from_rgba_lock = self.from_rgba.lock().await;
        let from_rgba = from_rgba_lock.get_or_insert(context.as_ref().unwrap().create_from_rgba(
            &VideoFormat::YUV420p,
            &ColourSpace::sRGB,
            1920,
            1080,
            InterlaceMode::Progressive,
        ));
        let mut start_lock = self.start.lock().await;
        let start = start_lock.get_or_insert(Instant::now());

        let (_output_id, video_frame) =
            video_frames.values().next().unwrap_or(&black_frame).clone();
        let video_frame = from_rgba.process_frame(video_frame).await;

        let (_output_id, audio_frame) = audio_frames.values().next().unwrap_or(&silence_frame);

        let copy_context = frame_context.submit().await;

        let video_frame = from_rgba.copy_frame(&copy_context, video_frame).await;
        let video_frame: Vec<u8> = video_frame.iter().flatten().cloned().collect();

        let now = Instant::now();
        let time = now - *start;
        let ms = time.as_secs() * 1000 + time.subsec_millis() as u64;
        let video_frames = {
            let mut vpx_lock = self.vpx.lock().await;
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
            let mut audio_encoder_lock = self.opus.lock().await;
            let audio_encoder = audio_encoder_lock.get_or_insert_with(|| {
                opus::Encoder::new(48000, opus::Channels::Mono, opus::Application::Audio).unwrap()
            });
            let fr: Vec<i16> = audio_frame
                .audio_buffers
                .first()
                .unwrap()
                .iter()
                .map(|s| f32::floor(*s * i16::MAX as f32) as i16)
                .collect();

            const OUT_SIZE: usize = 256; // TODO: This is only correct for mono up to 48kHz
            let mut out = vec![0u8; OUT_SIZE];
            let bytes = audio_encoder.encode(&fr, &mut out).unwrap();

            out[0..bytes].to_vec()
        };

        interval.tick().await;

        let audio_tracks_lock = self.audio_tracks.lock().await;
        if let Some(audio_tracks) = &*audio_tracks_lock {
            for track in audio_tracks.lock().await.iter() {
                write_audio_to_track(track.clone(), audio_frame.clone().into()).await;
            }
        }

        for frame in video_frames {
            let video_tracks_lock = self.video_tracks.lock().await;
            if let Some(video_tracks) = &*video_tracks_lock {
                for track in video_tracks.lock().await.iter() {
                    write_video_to_track(track.clone(), frame.clone().into()).await;
                }
            }
        }
    }
}

async fn write_video_to_track<'a>(t: Arc<TrackLocalStaticSample>, data: Bytes) {
    t.write_sample(&Sample {
        data,
        duration: Duration::from_millis(40),
        ..Default::default()
    })
    .await
    .unwrap();
}

async fn write_audio_to_track<'a>(t: Arc<TrackLocalStaticSample>, data: Bytes) {
    t.write_sample(&Sample {
        data,
        duration: Duration::from_millis(40),
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
        let mut video_tracks = state.video_tracks.lock().await;
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
        let mut audio_tracks = state.audio_tracks.lock().await;
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
