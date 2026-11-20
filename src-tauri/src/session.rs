use crate::models::{
    ChatMessage, IncomingRequest, OutgoingRequest, Peer, RequestClosed, SessionEnded, SessionInfo,
    now_ms,
};
use crate::protocol::{
    Control, MAX_CHUNK_FRAME, MAX_CONTROL_FRAME, PROTOCOL_VERSION, encode_control, is_lan_ip,
    read_control, read_frame, write_control,
};
use crate::state::{AppState, PendingIncoming, SharedState, emit};
use crate::transfer::{self, Transfer};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

pub struct Session {
    pub id: String,
    pub peer: Peer,
    pub started_at: i64,
    pub initiated_by_me: bool,
    pub writer: mpsc::Sender<Vec<u8>>,
    pub closed: AtomicBool,
    pub reader_task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    pub writer_task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    pub transfers: Mutex<HashMap<String, Arc<Transfer>>>,
    pub incoming_seq: Mutex<HashMap<u64, Arc<Transfer>>>,
    pub seq_counter: AtomicU64,
    pub file_gate: tokio::sync::Mutex<()>,
}

impl Session {
    fn new(id: String, peer: Peer, initiated_by_me: bool, writer: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            id,
            peer,
            started_at: now_ms(),
            initiated_by_me,
            writer,
            closed: AtomicBool::new(false),
            reader_task: Mutex::new(None),
            writer_task: Mutex::new(None),
            transfers: Mutex::new(HashMap::new()),
            incoming_seq: Mutex::new(HashMap::new()),
            seq_counter: AtomicU64::new(1),
            file_gate: tokio::sync::Mutex::new(()),
        }
    }

    pub fn info(&self) -> SessionInfo {
        SessionInfo {
            id: self.id.clone(),
            peer: self.peer.clone(),
            started_at: self.started_at,
            initiated_by_me: self.initiated_by_me,
        }
    }

    pub async fn send_control(&self, message: &Control) -> Result<(), String> {
        self.writer
            .send(encode_control(message))
            .await
            .map_err(|_| "Connection lost".to_string())
    }
}

pub fn clean_cache(state: &AppState) {
    state.clear_cache();
}

pub fn start_tcp_server(
    app: tauri::AppHandle,
    state: SharedState,
    listener: std::net::TcpListener,
) {
    tauri::async_runtime::spawn(async move {
        listener.set_nonblocking(true).ok();
        let listener = match TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("[localink] failed to start tcp listener: {error}");
                return;
            }
        };

        loop {
            match listener.accept().await {
                Ok((stream, address)) => {
                    if !is_lan_ip(address.ip()) {
                        continue;
                    }
                    let permit = match state.handshake_slots.clone().try_acquire_owned() {
                        Ok(permit) => permit,
                        Err(_) => continue,
                    };
                    let app = app.clone();
                    let state = state.clone();
                    tauri::async_runtime::spawn(async move {
                        let _permit = permit;
                        handle_incoming(app, state, stream, address).await;
                    });
                }
                Err(error) => {
                    eprintln!("[localink] tcp accept error: {error}");
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            }
        }
    });
}

async fn handle_incoming(
    app: tauri::AppHandle,
    state: SharedState,
    mut stream: TcpStream,
    address: SocketAddr,
) {
    let _ = stream.set_nodelay(true);

    let hello = match tokio::time::timeout(Duration::from_secs(10), read_control(&mut stream)).await
    {
        Ok(Ok(hello)) => hello,
        _ => return,
    };

    let Control::ClientHello {
        protocol,
        request_id,
        device,
        tcp_port,
    } = hello
    else {
        return;
    };

    if protocol != PROTOCOL_VERSION
        || request_id.is_empty()
        || request_id.len() > 64
        || device.id.trim().is_empty()
        || device.id == state.device_info().id
    {
        let _ = write_control(
            &mut stream,
            &Control::ServerHello {
                protocol: PROTOCOL_VERSION,
                accepted: false,
                session_id: None,
                device: None,
                reason: Some("Incompatible device or protocol version".to_string()),
            },
        )
        .await;
        return;
    }

    if !state.is_discoverable() {
        let _ = reject(&mut stream, "The other device has discovery turned off").await;
        return;
    }
    if state.has_session() {
        let _ = reject(&mut stream, "The other device is already in a session").await;
        return;
    }
    if !state.pending_outgoing.lock().unwrap().is_empty() {
        let _ = reject(
            &mut stream,
            "The other device is already requesting a connection",
        )
        .await;
        return;
    }
    if !state.pending_incoming.lock().unwrap().is_empty() {
        let _ = reject(
            &mut stream,
            "The other device is handling another connection request",
        )
        .await;
        return;
    }

    let peer = Peer {
        id: device.id,
        name: device.name,
        platform: device.platform,
        ip: address.ip().to_string(),
        port: tcp_port,
        busy: false,
        last_seen: now_ms(),
    };

    let (decision_tx, decision_rx) = oneshot::channel::<bool>();
    state.pending_incoming.lock().unwrap().insert(
        request_id.clone(),
        PendingIncoming {
            responder: decision_tx,
        },
    );

    emit(
        &app,
        "incoming-request",
        IncomingRequest {
            request_id: request_id.clone(),
            peer: peer.clone(),
        },
    );

    let decision = tokio::select! {
        result = decision_rx => result.ok(),
        _ = tokio::time::sleep(Duration::from_secs(45)) => None,
        _ = wait_peer_closed(&stream) => None,
    };

    state.pending_incoming.lock().unwrap().remove(&request_id);
    emit(
        &app,
        "incoming-request-closed",
        RequestClosed {
            request_id: request_id.clone(),
        },
    );

    let Some(accepted) = decision else {
        let _ = reject(
            &mut stream,
            "The connection request timed out or was cancelled",
        )
        .await;
        return;
    };

    if !accepted {
        let _ = reject(
            &mut stream,
            "The other device declined the connection request",
        )
        .await;
        return;
    }

    if state.has_session() {
        let _ = reject(&mut stream, "The other device is already in a session").await;
        return;
    }

    let session_id = Uuid::new_v4().to_string();
    let device = state.device_info();
    if write_control(
        &mut stream,
        &Control::ServerHello {
            protocol: PROTOCOL_VERSION,
            accepted: true,
            session_id: Some(session_id.clone()),
            device: Some(device),
            reason: None,
        },
    )
    .await
    .is_err()
    {
        return;
    }

    if let Err(error) = activate_session(app, state, stream, peer, false, session_id).await {
        eprintln!("[localink] failed to activate incoming session: {error}");
    }
}

async fn wait_peer_closed(stream: &TcpStream) -> bool {
    let mut probe = [0u8; 1];
    loop {
        match tokio::time::timeout(Duration::from_millis(500), stream.peek(&mut probe)).await {
            Ok(Ok(0)) | Ok(Err(_)) => return true,
            Ok(Ok(_)) => return true,
            Err(_) => continue,
        }
    }
}

async fn reject(stream: &mut TcpStream, reason: &str) -> std::io::Result<()> {
    write_control(
        stream,
        &Control::ServerHello {
            protocol: PROTOCOL_VERSION,
            accepted: false,
            session_id: None,
            device: None,
            reason: Some(reason.to_string()),
        },
    )
    .await
}

pub fn spawn_outgoing(
    app: tauri::AppHandle,
    state: SharedState,
    request_id: String,
    peer: Peer,
) -> (tauri::async_runtime::JoinHandle<()>, oneshot::Sender<()>) {
    let (start_tx, start_rx) = oneshot::channel();
    let task = tauri::async_runtime::spawn(async move {
        if start_rx.await.is_err() {
            return;
        }
        let result = outgoing_handshake(&app, &state, &request_id, &peer).await;
        if let Err((status, reason)) = result
            && take_outgoing(&state, &request_id).is_some()
        {
            emit(
                &app,
                "outgoing-request-updated",
                OutgoingRequest {
                    request_id,
                    peer,
                    status,
                    reason,
                },
            );
        }
    });
    (task, start_tx)
}

async fn outgoing_handshake(
    app: &tauri::AppHandle,
    state: &SharedState,
    request_id: &str,
    peer: &Peer,
) -> Result<(), (String, Option<String>)> {
    let target = SocketAddr::new(
        peer.ip.parse().map_err(|_| {
            (
                "failed".to_string(),
                Some("Invalid device address".to_string()),
            )
        })?,
        peer.port,
    );

    let mut stream = tokio::time::timeout(Duration::from_secs(6), TcpStream::connect(target))
        .await
        .map_err(|_| {
            (
                "failed".to_string(),
                Some("Connection timed out".to_string()),
            )
        })?
        .map_err(|_| {
            (
                "failed".to_string(),
                Some("Couldn't connect to the other device".to_string()),
            )
        })?;
    let _ = stream.set_nodelay(true);

    let device = state.device_info();
    write_control(
        &mut stream,
        &Control::ClientHello {
            protocol: PROTOCOL_VERSION,
            request_id: request_id.to_string(),
            device,
            tcp_port: state.tcp_port,
        },
    )
    .await
    .map_err(|_| {
        (
            "failed".to_string(),
            Some("Couldn't send the connection request".to_string()),
        )
    })?;

    let reply = tokio::time::timeout(Duration::from_secs(60), read_control(&mut stream))
        .await
        .map_err(|_| {
            (
                "timeout".to_string(),
                Some("The other device didn't respond".to_string()),
            )
        })?
        .map_err(|_| {
            (
                "failed".to_string(),
                Some("The other device closed the connection".to_string()),
            )
        })?;

    let Control::ServerHello {
        protocol,
        accepted,
        session_id,
        device: server_device,
        reason,
    } = reply
    else {
        return Err((
            "failed".to_string(),
            Some("Invalid response from the other device".to_string()),
        ));
    };

    if protocol != PROTOCOL_VERSION {
        return Err((
            "failed".to_string(),
            Some("Localink versions are incompatible".to_string()),
        ));
    }

    if !accepted {
        return Err((
            "rejected".to_string(),
            reason.or_else(|| Some("The other device declined the connection request".to_string())),
        ));
    }

    let Some(server_device) = server_device else {
        return Err((
            "failed".to_string(),
            Some("The other device's info is missing".to_string()),
        ));
    };
    let Some(session_id) = session_id else {
        return Err((
            "failed".to_string(),
            Some("Session identifier is missing".to_string()),
        ));
    };

    let mut active_peer = peer.clone();
    active_peer.name = server_device.name;
    active_peer.platform = server_device.platform;
    active_peer.busy = false;
    active_peer.last_seen = now_ms();

    activate_session(
        app.clone(),
        state.clone(),
        stream,
        active_peer,
        true,
        session_id,
    )
    .await
    .map_err(|error| ("failed".to_string(), Some(error)))?;

    take_outgoing(state, request_id);
    Ok(())
}

pub async fn activate_session(
    app: tauri::AppHandle,
    state: SharedState,
    stream: TcpStream,
    peer: Peer,
    initiated_by_me: bool,
    session_id: String,
) -> Result<Arc<Session>, String> {
    let _ = stream.set_nodelay(true);
    let (reader, writer) = stream.into_split();
    let (writer_tx, writer_rx) = mpsc::channel::<Vec<u8>>(16);
    let session = Arc::new(Session::new(
        session_id.clone(),
        peer.clone(),
        initiated_by_me,
        writer_tx,
    ));

    {
        let mut sessions = state.sessions.lock().unwrap();
        if !sessions.is_empty() {
            return Err("A session is already active".to_string());
        }
        sessions.insert(session_id.clone(), session.clone());
    }
    state.set_busy(true);

    let writer_task = tauri::async_runtime::spawn(writer_loop(
        app.clone(),
        state.clone(),
        session.clone(),
        writer,
        writer_rx,
    ));
    *session.writer_task.lock().unwrap() = Some(writer_task);

    let reader_task = tauri::async_runtime::spawn(reader_loop(
        app.clone(),
        state.clone(),
        session.clone(),
        reader,
    ));
    *session.reader_task.lock().unwrap() = Some(reader_task);

    emit(&app, "session-started", session.info());
    Ok(session)
}

async fn writer_loop(
    app: tauri::AppHandle,
    state: SharedState,
    session: Arc<Session>,
    mut writer: OwnedWriteHalf,
    mut receiver: mpsc::Receiver<Vec<u8>>,
) {
    while let Some(frame) = receiver.recv().await {
        if session.closed.load(Ordering::SeqCst) {
            break;
        }
        if writer.write_all(&frame).await.is_err() {
            schedule_terminate(
                app.clone(),
                state.clone(),
                session.id.clone(),
                "error",
                Some("Network connection lost".to_string()),
            );
            return;
        }
    }
    let _ = writer.shutdown().await;
    if !session.closed.load(Ordering::SeqCst) {
        schedule_terminate(
            app.clone(),
            state.clone(),
            session.id.clone(),
            "error",
            Some("Network connection lost".to_string()),
        );
    }
}

async fn reader_loop(
    app: tauri::AppHandle,
    state: SharedState,
    session: Arc<Session>,
    mut reader: OwnedReadHalf,
) {
    loop {
        let frame = read_frame(&mut reader, MAX_CHUNK_FRAME).await;
        let (kind, payload) = match frame {
            Ok(frame) => frame,
            Err(_) => {
                schedule_terminate(app.clone(), state.clone(), session.id.clone(), "peer", None);
                return;
            }
        };

        match kind {
            crate::protocol::FRAME_CONTROL => {
                if payload.len() > MAX_CONTROL_FRAME {
                    schedule_terminate(
                        app.clone(),
                        state.clone(),
                        session.id.clone(),
                        "error",
                        Some("Received an oversized protocol message".to_string()),
                    );
                    return;
                }
                let message: Control = match serde_json::from_slice(&payload) {
                    Ok(message) => message,
                    Err(_) => {
                        schedule_terminate(
                            app.clone(),
                            state.clone(),
                            session.id.clone(),
                            "error",
                            Some("Couldn't parse the protocol message".to_string()),
                        );
                        return;
                    }
                };
                if !handle_control(&app, &state, &session, message).await {
                    return;
                }
            }
            crate::protocol::FRAME_CHUNK => {
                transfer::handle_chunk(&app, &session, &payload).await;
            }
            _ => {
                schedule_terminate(
                    app.clone(),
                    state.clone(),
                    session.id.clone(),
                    "error",
                    Some("Received an unknown data frame".to_string()),
                );
                return;
            }
        }
    }
}

async fn handle_control(
    app: &tauri::AppHandle,
    state: &SharedState,
    session: &Arc<Session>,
    message: Control,
) -> bool {
    match message {
        Control::Text { id, text, .. } => {
            if text.is_empty() || text.len() > 64 * 1024 {
                return true;
            }
            let id = if id.is_empty() || id.len() > 64 {
                Uuid::new_v4().to_string()
            } else {
                id
            };
            emit(
                app,
                "message-added",
                ChatMessage {
                    id,
                    session_id: session.id.clone(),
                    direction: "in".to_string(),
                    kind: "text".to_string(),
                    text: Some(text),
                    transfer: None,
                    at: now_ms(),
                },
            );
            true
        }
        Control::FileOffer {
            id,
            seq,
            name,
            size,
        } => {
            transfer::handle_file_offer(app, state, session, id, seq, name, size).await;
            true
        }
        Control::FileAccept { id, offset } => {
            transfer::handle_file_accept(app, session, &id, offset).await;
            true
        }
        Control::FileReject { id, reason } => {
            transfer::handle_file_reject(app, session, &id, reason).await;
            true
        }
        Control::FileComplete { id } => {
            transfer::handle_file_complete(app, state, session, &id).await;
            true
        }
        Control::FileReceived { id } => {
            transfer::handle_file_received(app, session, &id).await;
            true
        }
        Control::TransferFailed { id, reason } => {
            transfer::handle_transfer_failed(app, session, &id, reason).await;
            true
        }
        Control::PauseRequest { id } => {
            transfer::handle_pause_request(app, session, &id).await;
            true
        }
        Control::ResumeRequest { id } => {
            transfer::handle_resume_request(app, session, &id).await;
            true
        }
        Control::TransferPaused { id } => {
            transfer::handle_transfer_paused(app, session, &id).await;
            true
        }
        Control::TransferResumed { id } => {
            transfer::handle_transfer_resumed(app, session, &id).await;
            true
        }
        Control::TransferCancel { id } => {
            transfer::handle_peer_cancel(app, session, &id).await;
            true
        }
        Control::ClientHello { .. } | Control::ServerHello { .. } => true,
    }
}

fn take_outgoing(state: &SharedState, request_id: &str) -> Option<Peer> {
    state
        .pending_outgoing
        .lock()
        .unwrap()
        .remove(request_id)
        .map(|pending| pending.peer)
}

fn schedule_terminate(
    app: tauri::AppHandle,
    state: SharedState,
    session_id: String,
    reason: &'static str,
    message: Option<String>,
) {
    tauri::async_runtime::spawn(async move {
        terminate_session(&app, &state, &session_id, reason, message).await;
    });
}

pub async fn terminate_session(
    app: &tauri::AppHandle,
    state: &SharedState,
    session_id: &str,
    reason: &str,
    message: Option<String>,
) {
    let session = {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.remove(session_id)
    };
    let Some(session) = session else {
        return;
    };
    if session.closed.swap(true, Ordering::SeqCst) {
        return;
    }

    if let Some(task) = session.reader_task.lock().unwrap().take() {
        task.abort();
    }
    if let Some(task) = session.writer_task.lock().unwrap().take() {
        task.abort();
    }

    transfer::abort_all(state, &session).await;
    state.set_busy(false);

    emit(
        app,
        "session-ended",
        SessionEnded {
            session_id: session.id.clone(),
            reason: reason.to_string(),
            message,
        },
    );
}

pub fn active_session(state: &AppState) -> Option<Arc<Session>> {
    state.sessions.lock().unwrap().values().next().cloned()
}
