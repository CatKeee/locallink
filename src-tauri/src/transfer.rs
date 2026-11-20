use crate::models::{ChatMessage, Direction, TransferDto, now_ms};
use crate::protocol::{CHUNK_SIZE, Control, encode_chunk};
use crate::session::Session;
use crate::state::{SharedState, emit};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::sync::{Notify, watch};
use uuid::Uuid;

pub const T_WAITING: u8 = 0;
pub const T_TRANSFERRING: u8 = 1;
pub const T_PAUSED: u8 = 2;
pub const T_COMPLETED: u8 = 3;
pub const T_CANCELLED: u8 = 4;
pub const T_FAILED: u8 = 5;

#[derive(Debug, Clone)]
pub enum OfferDecision {
    Accepted(u64),
    Rejected(String),
    Cancelled,
}

pub struct Transfer {
    pub id: String,
    pub session_id: String,
    pub seq: u64,
    pub direction: Direction,
    pub name: String,
    pub size: u64,
    pub source_path: Option<PathBuf>,
    pub temp_path: Option<PathBuf>,
    pub final_path: Mutex<Option<PathBuf>>,
    pub transferred: AtomicU64,
    pub state: AtomicU8,
    pub error: Mutex<Option<String>>,
    pub speed_bps: AtomicU64,
    pub last_sample: Mutex<(u64, Instant)>,
    pub paused: AtomicBool,
    pub cancelled: AtomicBool,
    pub pause_notify: Notify,
    pub offer_tx: watch::Sender<Option<OfferDecision>>,
    pub incoming_file: tokio::sync::Mutex<Option<File>>,
}

impl Transfer {
    pub fn state(&self) -> u8 {
        self.state.load(Ordering::SeqCst)
    }

    fn reset_sample(&self) {
        let bytes = self.transferred.load(Ordering::SeqCst);
        *self.last_sample.lock().unwrap() = (bytes, Instant::now());
        self.speed_bps.store(0, Ordering::SeqCst);
    }
}

fn state_name(state: u8) -> &'static str {
    match state {
        T_WAITING => "waiting",
        T_TRANSFERRING => "transferring",
        T_PAUSED => "paused",
        T_COMPLETED => "completed",
        T_CANCELLED => "cancelled",
        T_FAILED => "failed",
        _ => "failed",
    }
}

fn is_terminal(state: u8) -> bool {
    matches!(state, T_COMPLETED | T_CANCELLED | T_FAILED)
}

pub fn snapshot(transfer: &Transfer) -> TransferDto {
    let transferred = transfer
        .transferred
        .load(Ordering::SeqCst)
        .min(transfer.size);
    let state = transfer.state();
    let speed = if state == T_TRANSFERRING {
        transfer.speed_bps.load(Ordering::SeqCst)
    } else {
        0
    };
    let remaining = transfer.size.saturating_sub(transferred);
    let eta_secs = if speed > 0 && remaining > 0 {
        Some(((remaining as f64) / (speed as f64)).ceil() as u64)
    } else {
        None
    };
    let final_path = transfer.final_path.lock().unwrap().clone();
    let path = final_path
        .or_else(|| transfer.source_path.clone())
        .map(|path| path.to_string_lossy().to_string());

    TransferDto {
        id: transfer.id.clone(),
        session_id: transfer.session_id.clone(),
        direction: transfer.direction.as_str().to_string(),
        name: transfer.name.clone(),
        size: transfer.size,
        transferred,
        state: state_name(state).to_string(),
        speed_bps: speed,
        eta_secs,
        path,
        error: transfer.error.lock().unwrap().clone(),
    }
}

pub fn emit_update(app: &tauri::AppHandle, transfer: &Transfer) {
    emit(app, "transfer-updated", snapshot(transfer));
}

pub fn message_for(transfer: &Transfer) -> ChatMessage {
    ChatMessage {
        id: transfer.id.clone(),
        session_id: transfer.session_id.clone(),
        direction: transfer.direction.as_str().to_string(),
        kind: "file".to_string(),
        text: None,
        transfer: Some(snapshot(transfer)),
        at: now_ms(),
    }
}

#[allow(clippy::too_many_arguments)]
fn new_transfer(
    id: String,
    session: &Session,
    seq: u64,
    direction: Direction,
    name: String,
    size: u64,
    source_path: Option<PathBuf>,
    temp_path: Option<PathBuf>,
) -> Arc<Transfer> {
    let (offer_tx, _) = watch::channel(None);
    Arc::new(Transfer {
        id,
        session_id: session.id.clone(),
        seq,
        direction,
        name,
        size,
        source_path,
        temp_path,
        final_path: Mutex::new(None),
        transferred: AtomicU64::new(0),
        state: AtomicU8::new(T_WAITING),
        error: Mutex::new(None),
        speed_bps: AtomicU64::new(0),
        last_sample: Mutex::new((0, Instant::now())),
        paused: AtomicBool::new(false),
        cancelled: AtomicBool::new(false),
        pause_notify: Notify::new(),
        offer_tx,
        incoming_file: tokio::sync::Mutex::new(None),
    })
}

pub fn register(state: &SharedState, session: &Session, transfer: Arc<Transfer>) {
    session
        .transfers
        .lock()
        .unwrap()
        .insert(transfer.id.clone(), transfer.clone());
    state
        .transfers
        .lock()
        .unwrap()
        .insert(transfer.id.clone(), transfer);
}

pub fn prepare_outgoing(
    session: &Session,
    name: String,
    size: u64,
    source_path: PathBuf,
) -> (Arc<Transfer>, ChatMessage) {
    let seq = session.seq_counter.fetch_add(1, Ordering::SeqCst);
    let transfer = new_transfer(
        Uuid::new_v4().to_string(),
        session,
        seq,
        Direction::Out,
        name,
        size,
        Some(source_path),
        None,
    );
    let message = message_for(&transfer);
    (transfer, message)
}

pub fn start_progress_ticker(app: tauri::AppHandle, state: SharedState) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;
            let transfers: Vec<Arc<Transfer>> =
                state.transfers.lock().unwrap().values().cloned().collect();

            for transfer in transfers {
                if transfer.state() != T_TRANSFERRING {
                    continue;
                }

                let now = Instant::now();
                let bytes = transfer.transferred.load(Ordering::SeqCst);
                let instant = {
                    let mut sample = transfer.last_sample.lock().unwrap();
                    let elapsed = now.duration_since(sample.1).as_secs_f64();
                    let delta = bytes.saturating_sub(sample.0) as f64;
                    *sample = (bytes, now);
                    if elapsed > 0.05 { delta / elapsed } else { 0.0 }
                };

                let previous = transfer.speed_bps.load(Ordering::SeqCst) as f64;
                let speed = if instant > 0.0 {
                    previous * 0.5 + instant * 0.5
                } else {
                    previous * 0.7
                };
                transfer.speed_bps.store(speed as u64, Ordering::SeqCst);
                emit_update(&app, &transfer);
            }
        }
    });
}

pub async fn outgoing_file_task(
    app: tauri::AppHandle,
    session: Arc<Session>,
    transfer: Arc<Transfer>,
) {
    let mut decision_rx = transfer.offer_tx.subscribe();

    if session
        .send_control(&Control::FileOffer {
            id: transfer.id.clone(),
            seq: transfer.seq,
            name: transfer.name.clone(),
            size: transfer.size,
        })
        .await
        .is_err()
    {
        fail(&app, &session, &transfer, "Connection lost").await;
        return;
    }

    let decision = tokio::select! {
        result = decision_rx.changed() => {
            if result.is_err() {
                return;
            }
            decision_rx.borrow_and_update().clone()
        }
        _ = tokio::time::sleep(Duration::from_secs(30)) => None,
    };

    let Some(decision) = decision else {
        if !transfer.cancelled.load(Ordering::SeqCst) {
            fail(
                &app,
                &session,
                &transfer,
                "The recipient didn't accept the file",
            )
            .await;
            let _ = session
                .send_control(&Control::TransferCancel {
                    id: transfer.id.clone(),
                })
                .await;
        }
        return;
    };

    let offset = match decision {
        OfferDecision::Accepted(offset) => offset,
        OfferDecision::Rejected(reason) => {
            fail(&app, &session, &transfer, &reason).await;
            return;
        }
        OfferDecision::Cancelled => return,
    };

    if offset > transfer.size {
        fail(
            &app,
            &session,
            &transfer,
            "The recipient returned an invalid resume position",
        )
        .await;
        return;
    }

    let _gate = session.file_gate.lock().await;
    if transfer.cancelled.load(Ordering::SeqCst) || session.closed.load(Ordering::SeqCst) {
        return;
    }

    let Some(source_path) = transfer.source_path.clone() else {
        fail(&app, &session, &transfer, "Couldn't find the source file").await;
        return;
    };

    let mut file = match File::open(&source_path).await {
        Ok(file) => file,
        Err(_) => {
            fail(&app, &session, &transfer, "Couldn't read the file").await;
            return;
        }
    };
    if offset > 0 && file.seek(SeekFrom::Start(offset)).await.is_err() {
        fail(
            &app,
            &session,
            &transfer,
            "Couldn't resume reading from the current position",
        )
        .await;
        return;
    }

    transfer.transferred.store(offset, Ordering::SeqCst);
    transfer.state.store(T_TRANSFERRING, Ordering::SeqCst);
    transfer.reset_sample();
    emit_update(&app, &transfer);

    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut position = offset;

    while position < transfer.size {
        if session.closed.load(Ordering::SeqCst) {
            return;
        }
        if transfer.cancelled.load(Ordering::SeqCst) {
            return;
        }
        wait_if_paused(&session, &transfer).await;
        if transfer.cancelled.load(Ordering::SeqCst) || session.closed.load(Ordering::SeqCst) {
            return;
        }

        let read = match file.read(&mut buffer).await {
            Ok(0) => break,
            Ok(read) => read,
            Err(_) => {
                fail(&app, &session, &transfer, "Error while reading the file").await;
                return;
            }
        };

        let frame = encode_chunk(transfer.seq, position, &buffer[..read]);
        if session.writer.send(frame).await.is_err() {
            fail(&app, &session, &transfer, "Network connection lost").await;
            return;
        }

        position += read as u64;
        transfer.transferred.store(position, Ordering::SeqCst);
    }

    if position < transfer.size {
        fail(
            &app,
            &session,
            &transfer,
            "The file changed while it was being sent",
        )
        .await;
        return;
    }

    if session
        .send_control(&Control::FileComplete {
            id: transfer.id.clone(),
        })
        .await
        .is_err()
    {
        fail(&app, &session, &transfer, "Network connection lost").await;
        return;
    }
    drop(_gate);

    transfer.speed_bps.store(0, Ordering::SeqCst);
    emit_update(&app, &transfer);

    // Keep the transfer at 100% until the receiver confirms that the file
    // was flushed and moved into the Downloads folder.
    for _ in 0..120 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let state = transfer.state();
        if state == T_COMPLETED || is_terminal(state) {
            return;
        }
        if session.closed.load(Ordering::SeqCst) {
            return;
        }
    }
    fail(
        &app,
        &session,
        &transfer,
        "Timed out waiting for the recipient to confirm the file",
    )
    .await;
}

async fn wait_if_paused(session: &Session, transfer: &Transfer) {
    while transfer.paused.load(Ordering::SeqCst) {
        if transfer.cancelled.load(Ordering::SeqCst) || session.closed.load(Ordering::SeqCst) {
            return;
        }
        let notified = transfer.pause_notify.notified();
        tokio::select! {
            _ = notified => {}
            _ = tokio::time::sleep(Duration::from_millis(250)) => {}
        }
    }
}

async fn fail(app: &tauri::AppHandle, session: &Session, transfer: &Transfer, reason: &str) {
    if is_terminal(transfer.state()) {
        return;
    }
    if transfer.direction == Direction::In {
        cleanup_incoming_file(session, transfer).await;
    }
    transfer.state.store(T_FAILED, Ordering::SeqCst);
    transfer.speed_bps.store(0, Ordering::SeqCst);
    *transfer.error.lock().unwrap() = Some(reason.to_string());
    transfer.cancelled.store(true, Ordering::SeqCst);
    transfer.paused.store(false, Ordering::SeqCst);
    transfer.pause_notify.notify_waiters();
    let _ = transfer.offer_tx.send(Some(OfferDecision::Cancelled));
    let _ = session
        .send_control(&Control::TransferFailed {
            id: transfer.id.clone(),
            reason: reason.to_string(),
        })
        .await;
    emit_update(app, transfer);
}

async fn mark_failed(app: &tauri::AppHandle, session: &Session, transfer: &Transfer, reason: &str) {
    if is_terminal(transfer.state()) {
        return;
    }
    if transfer.direction == Direction::In {
        cleanup_incoming_file(session, transfer).await;
    }
    transfer.state.store(T_FAILED, Ordering::SeqCst);
    transfer.speed_bps.store(0, Ordering::SeqCst);
    *transfer.error.lock().unwrap() = Some(reason.to_string());
    transfer.cancelled.store(true, Ordering::SeqCst);
    transfer.paused.store(false, Ordering::SeqCst);
    transfer.pause_notify.notify_waiters();
    let _ = transfer.offer_tx.send(Some(OfferDecision::Cancelled));
    emit_update(app, transfer);
}

pub async fn handle_file_offer(
    app: &tauri::AppHandle,
    state: &SharedState,
    session: &Arc<Session>,
    id: String,
    seq: u64,
    raw_name: String,
    size: u64,
) {
    if Uuid::parse_str(&id).is_err() || seq == 0 {
        return;
    }
    if session.transfers.lock().unwrap().len() >= 64 {
        let _ = session
            .send_control(&Control::FileReject {
                id,
                reason: "Too many simultaneous transfers".to_string(),
            })
            .await;
        return;
    }
    if session.transfers.lock().unwrap().contains_key(&id) {
        return;
    }
    if session.incoming_seq.lock().unwrap().contains_key(&seq) {
        return;
    }

    let reserve = size.saturating_add(64 * 1024 * 1024);
    let insufficient_space = fs2::available_space(&state.transfers_dir)
        .map(|available| available < reserve)
        .unwrap_or(false);
    if insufficient_space {
        let _ = session
            .send_control(&Control::FileReject {
                id,
                reason: "The recipient doesn't have enough disk space".to_string(),
            })
            .await;
        return;
    }

    let name = sanitize_file_name(&raw_name);
    let temp_path = state.transfers_dir.join(format!("{id}.part"));
    let transfer = new_transfer(
        id.clone(),
        session,
        seq,
        Direction::In,
        name,
        size,
        None,
        Some(temp_path.clone()),
    );

    register(state, session, transfer.clone());
    session
        .incoming_seq
        .lock()
        .unwrap()
        .insert(seq, transfer.clone());

    emit(app, "message-added", message_for(&transfer));

    let mut file = match OpenOptions::new()
        .create(true)
        .truncate(false) // keep an existing partial file for offset-based resume
        .read(true)
        .write(true)
        .open(&temp_path)
        .await
    {
        Ok(file) => file,
        Err(error) => {
            fail(
                app,
                session,
                &transfer,
                &format!("Couldn't create cache file: {error}"),
            )
            .await;
            return;
        }
    };

    let mut offset = file.metadata().await.map(|meta| meta.len()).unwrap_or(0);
    if offset > size {
        let _ = file.set_len(0).await;
        let _ = file.seek(SeekFrom::Start(0)).await;
        offset = 0;
    }
    transfer.transferred.store(offset, Ordering::SeqCst);
    if offset > 0 {
        let _ = file.seek(SeekFrom::Start(offset)).await;
    }
    *transfer.incoming_file.lock().await = Some(file);

    if session
        .send_control(&Control::FileAccept {
            id: transfer.id.clone(),
            offset,
        })
        .await
        .is_err()
    {
        fail(app, session, &transfer, "Connection lost").await;
        return;
    }
    emit_update(app, &transfer);
}

pub async fn handle_chunk(app: &tauri::AppHandle, session: &Arc<Session>, payload: &[u8]) {
    if payload.len() < 16 || payload.len() > 1024 * 1024 + 16 {
        return;
    }
    let seq = u64::from_be_bytes(payload[0..8].try_into().unwrap());
    let offset = u64::from_be_bytes(payload[8..16].try_into().unwrap());
    let data = &payload[16..];

    let transfer = match session.incoming_seq.lock().unwrap().get(&seq).cloned() {
        Some(transfer) => transfer,
        None => return,
    };
    if transfer.cancelled.load(Ordering::SeqCst) || is_terminal(transfer.state()) {
        return;
    }
    if offset > transfer.size || offset + data.len() as u64 > transfer.size {
        return;
    }

    let write_failed = {
        let mut guard = transfer.incoming_file.lock().await;
        let Some(file) = guard.as_mut() else {
            return;
        };
        file.seek(SeekFrom::Start(offset)).await.is_err() || file.write_all(data).await.is_err()
    };

    if write_failed {
        fail(
            app,
            session,
            &transfer,
            "Failed to write the temporary file",
        )
        .await;
        return;
    }

    transfer
        .transferred
        .store(offset + data.len() as u64, Ordering::SeqCst);
    if transfer.state() == T_WAITING {
        transfer.state.store(T_TRANSFERRING, Ordering::SeqCst);
        transfer.reset_sample();
    }
}

pub async fn handle_file_accept(
    app: &tauri::AppHandle,
    session: &Arc<Session>,
    id: &str,
    offset: u64,
) {
    let transfer = session.transfers.lock().unwrap().get(id).cloned();
    let Some(transfer) = transfer else {
        return;
    };
    if transfer.direction != Direction::Out || is_terminal(transfer.state()) {
        return;
    }
    if offset > transfer.size {
        fail(
            app,
            session,
            &transfer,
            "The recipient returned an invalid resume position",
        )
        .await;
        return;
    }
    let _ = transfer
        .offer_tx
        .send(Some(OfferDecision::Accepted(offset)));
}

pub async fn handle_file_reject(
    app: &tauri::AppHandle,
    session: &Arc<Session>,
    id: &str,
    reason: String,
) {
    let transfer = session.transfers.lock().unwrap().get(id).cloned();
    let Some(transfer) = transfer else {
        return;
    };
    if transfer.direction != Direction::Out || is_terminal(transfer.state()) {
        return;
    }
    if transfer
        .offer_tx
        .send(Some(OfferDecision::Rejected(sanitize_reason(&reason))))
        .is_err()
    {
        fail(app, session, &transfer, "The recipient declined the file").await;
    }
}

pub async fn handle_file_complete(
    app: &tauri::AppHandle,
    state: &SharedState,
    session: &Arc<Session>,
    id: &str,
) {
    let transfer = session.transfers.lock().unwrap().get(id).cloned();
    let Some(transfer) = transfer else {
        return;
    };
    if transfer.direction != Direction::In || is_terminal(transfer.state()) {
        return;
    }

    let file = transfer.incoming_file.lock().await.take();
    let mut file = match file {
        Some(file) => file,
        None => {
            fail(app, session, &transfer, "Receiving state was lost").await;
            return;
        }
    };

    if let Err(error) = file.flush().await {
        fail(
            app,
            session,
            &transfer,
            &format!("Failed to write the file: {error}"),
        )
        .await;
        cleanup_incoming_file(session, &transfer).await;
        return;
    }
    let size_on_disk = file.metadata().await.map(|meta| meta.len()).unwrap_or(0);
    drop(file);

    if size_on_disk != transfer.size {
        fail(app, session, &transfer, "The received file is incomplete").await;
        cleanup_incoming_file(session, &transfer).await;
        return;
    }

    let Some(temp_path) = transfer.temp_path.clone() else {
        fail(app, session, &transfer, "Temporary file path is missing").await;
        return;
    };
    let destination = unique_download_path(&state.downloads_dir, &transfer.name);

    if move_file(&temp_path, &destination).await.is_err() {
        fail(
            app,
            session,
            &transfer,
            "Couldn't save the file to Downloads",
        )
        .await;
        let _ = tokio::fs::remove_file(&temp_path).await;
        return;
    }

    transfer.transferred.store(transfer.size, Ordering::SeqCst);
    transfer.state.store(T_COMPLETED, Ordering::SeqCst);
    transfer.speed_bps.store(0, Ordering::SeqCst);
    *transfer.final_path.lock().unwrap() = Some(destination);
    *transfer.error.lock().unwrap() = None;
    session.incoming_seq.lock().unwrap().remove(&transfer.seq);
    emit_update(app, &transfer);
    let _ = session
        .send_control(&Control::FileReceived {
            id: transfer.id.clone(),
        })
        .await;
}

pub async fn handle_file_received(app: &tauri::AppHandle, session: &Arc<Session>, id: &str) {
    let transfer = session.transfers.lock().unwrap().get(id).cloned();
    let Some(transfer) = transfer else {
        return;
    };
    if transfer.direction != Direction::Out || is_terminal(transfer.state()) {
        return;
    }
    transfer.transferred.store(transfer.size, Ordering::SeqCst);
    transfer.state.store(T_COMPLETED, Ordering::SeqCst);
    transfer.speed_bps.store(0, Ordering::SeqCst);
    *transfer.error.lock().unwrap() = None;
    emit_update(app, &transfer);
}

pub async fn handle_transfer_failed(
    app: &tauri::AppHandle,
    session: &Arc<Session>,
    id: &str,
    reason: String,
) {
    let transfer = session.transfers.lock().unwrap().get(id).cloned();
    let Some(transfer) = transfer else {
        return;
    };
    mark_failed(app, session, &transfer, &sanitize_reason(&reason)).await;
}

pub async fn handle_pause_request(app: &tauri::AppHandle, session: &Arc<Session>, id: &str) {
    let transfer = session.transfers.lock().unwrap().get(id).cloned();
    let Some(transfer) = transfer else {
        return;
    };
    if transfer.direction != Direction::Out || is_terminal(transfer.state()) {
        return;
    }
    transfer.paused.store(true, Ordering::SeqCst);
    if transfer.state() == T_WAITING || transfer.state() == T_TRANSFERRING {
        transfer.state.store(T_PAUSED, Ordering::SeqCst);
    }
    transfer.speed_bps.store(0, Ordering::SeqCst);
    emit_update(app, &transfer);
    let _ = session
        .send_control(&Control::TransferPaused {
            id: transfer.id.clone(),
        })
        .await;
}

pub async fn handle_resume_request(app: &tauri::AppHandle, session: &Arc<Session>, id: &str) {
    let transfer = session.transfers.lock().unwrap().get(id).cloned();
    let Some(transfer) = transfer else {
        return;
    };
    if transfer.direction != Direction::Out || is_terminal(transfer.state()) {
        return;
    }
    transfer.paused.store(false, Ordering::SeqCst);
    if transfer.state() == T_PAUSED {
        transfer.state.store(T_TRANSFERRING, Ordering::SeqCst);
        transfer.reset_sample();
    }
    transfer.pause_notify.notify_waiters();
    emit_update(app, &transfer);
    let _ = session
        .send_control(&Control::TransferResumed {
            id: transfer.id.clone(),
        })
        .await;
}

pub async fn handle_transfer_paused(app: &tauri::AppHandle, session: &Arc<Session>, id: &str) {
    let transfer = session.transfers.lock().unwrap().get(id).cloned();
    let Some(transfer) = transfer else {
        return;
    };
    if transfer.direction != Direction::In || is_terminal(transfer.state()) {
        return;
    }
    transfer.paused.store(true, Ordering::SeqCst);
    transfer.state.store(T_PAUSED, Ordering::SeqCst);
    transfer.speed_bps.store(0, Ordering::SeqCst);
    emit_update(app, &transfer);
}

pub async fn handle_transfer_resumed(app: &tauri::AppHandle, session: &Arc<Session>, id: &str) {
    let transfer = session.transfers.lock().unwrap().get(id).cloned();
    let Some(transfer) = transfer else {
        return;
    };
    if transfer.direction != Direction::In || is_terminal(transfer.state()) {
        return;
    }
    transfer.paused.store(false, Ordering::SeqCst);
    transfer.state.store(T_TRANSFERRING, Ordering::SeqCst);
    transfer.reset_sample();
    emit_update(app, &transfer);
}

pub async fn pause(
    app: &tauri::AppHandle,
    session: &Arc<Session>,
    transfer: &Arc<Transfer>,
) -> Result<(), String> {
    if is_terminal(transfer.state()) || transfer.state() == T_PAUSED {
        return Ok(());
    }
    match transfer.direction {
        Direction::Out => {
            transfer.paused.store(true, Ordering::SeqCst);
            transfer.state.store(T_PAUSED, Ordering::SeqCst);
            transfer.speed_bps.store(0, Ordering::SeqCst);
            emit_update(app, transfer);
            session
                .send_control(&Control::TransferPaused {
                    id: transfer.id.clone(),
                })
                .await
        }
        Direction::In => {
            session
                .send_control(&Control::PauseRequest {
                    id: transfer.id.clone(),
                })
                .await?;
            transfer.paused.store(true, Ordering::SeqCst);
            transfer.state.store(T_PAUSED, Ordering::SeqCst);
            transfer.speed_bps.store(0, Ordering::SeqCst);
            emit_update(app, transfer);
            Ok(())
        }
    }
}

pub async fn resume(
    app: &tauri::AppHandle,
    session: &Arc<Session>,
    transfer: &Arc<Transfer>,
) -> Result<(), String> {
    if is_terminal(transfer.state()) {
        return Ok(());
    }
    match transfer.direction {
        Direction::Out => {
            transfer.paused.store(false, Ordering::SeqCst);
            if transfer.state() == T_PAUSED {
                transfer.state.store(T_TRANSFERRING, Ordering::SeqCst);
                transfer.reset_sample();
            }
            transfer.pause_notify.notify_waiters();
            emit_update(app, transfer);
            session
                .send_control(&Control::TransferResumed {
                    id: transfer.id.clone(),
                })
                .await
        }
        Direction::In => {
            session
                .send_control(&Control::ResumeRequest {
                    id: transfer.id.clone(),
                })
                .await?;
            transfer.paused.store(false, Ordering::SeqCst);
            transfer.state.store(T_TRANSFERRING, Ordering::SeqCst);
            transfer.reset_sample();
            emit_update(app, transfer);
            Ok(())
        }
    }
}

pub async fn cancel(
    app: &tauri::AppHandle,
    session: &Arc<Session>,
    transfer: &Arc<Transfer>,
) -> Result<(), String> {
    if is_terminal(transfer.state()) {
        return Ok(());
    }
    transfer.cancelled.store(true, Ordering::SeqCst);
    transfer.paused.store(false, Ordering::SeqCst);
    transfer.state.store(T_CANCELLED, Ordering::SeqCst);
    transfer.speed_bps.store(0, Ordering::SeqCst);
    transfer.pause_notify.notify_waiters();
    let _ = transfer.offer_tx.send(Some(OfferDecision::Cancelled));
    if transfer.direction == Direction::In {
        cleanup_incoming_file(session, transfer).await;
    }
    emit_update(app, transfer);
    let _ = session
        .send_control(&Control::TransferCancel {
            id: transfer.id.clone(),
        })
        .await;
    Ok(())
}

pub async fn handle_peer_cancel(app: &tauri::AppHandle, session: &Arc<Session>, id: &str) {
    let transfer = session.transfers.lock().unwrap().get(id).cloned();
    let Some(transfer) = transfer else {
        return;
    };
    if is_terminal(transfer.state()) {
        return;
    }
    transfer.cancelled.store(true, Ordering::SeqCst);
    transfer.paused.store(false, Ordering::SeqCst);
    transfer.state.store(T_CANCELLED, Ordering::SeqCst);
    transfer.speed_bps.store(0, Ordering::SeqCst);
    transfer.pause_notify.notify_waiters();
    let _ = transfer.offer_tx.send(Some(OfferDecision::Cancelled));
    if transfer.direction == Direction::In {
        cleanup_incoming_file(session, &transfer).await;
    }
    emit_update(app, &transfer);
}

pub async fn abort_all(state: &SharedState, session: &Arc<Session>) {
    let transfers: Vec<Arc<Transfer>> = session
        .transfers
        .lock()
        .unwrap()
        .values()
        .cloned()
        .collect();

    for transfer in transfers {
        transfer.cancelled.store(true, Ordering::SeqCst);
        transfer.paused.store(false, Ordering::SeqCst);
        if !is_terminal(transfer.state()) {
            transfer.state.store(T_CANCELLED, Ordering::SeqCst);
        }
        transfer.speed_bps.store(0, Ordering::SeqCst);
        transfer.pause_notify.notify_waiters();
        let _ = transfer.offer_tx.send(Some(OfferDecision::Cancelled));
        if transfer.direction == Direction::In {
            cleanup_incoming_file(session, &transfer).await;
        }
    }

    session.transfers.lock().unwrap().clear();
    session.incoming_seq.lock().unwrap().clear();
    state
        .transfers
        .lock()
        .unwrap()
        .retain(|_, transfer| transfer.session_id != session.id);
}

async fn cleanup_incoming_file(session: &Session, transfer: &Transfer) {
    if let Some(mut file) = transfer.incoming_file.lock().await.take() {
        let _ = file.flush().await;
    }
    if let Some(path) = transfer.temp_path.clone() {
        let _ = tokio::fs::remove_file(path).await;
    }
    session.incoming_seq.lock().unwrap().remove(&transfer.seq);
}

pub fn sanitize_file_name(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw).trim();
    let cleaned: String = base
        .chars()
        .filter(|character| {
            !character.is_control() && !matches!(character, ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    let mut name = if cleaned.is_empty() {
        "Untitled file".to_string()
    } else {
        cleaned
    };

    name = truncate_file_name(&name, 200);

    let stem = Path::new(&name)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    let reserved = matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if reserved {
        name.insert(0, '_');
    }
    name
}

fn truncate_file_name(name: &str, max_bytes: usize) -> String {
    if name.len() <= max_bytes {
        return name.to_string();
    }

    let path = Path::new(name);
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    let extension = if extension.len() <= 24 {
        extension
    } else {
        String::new()
    };
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string());
    let budget = max_bytes.saturating_sub(extension.len());
    let mut end = budget.min(stem.len());
    while end > 0 && !stem.is_char_boundary(end) {
        end -= 1;
    }
    let truncated = format!("{}{}", &stem[..end], extension);
    if truncated.is_empty() {
        "Untitled file".to_string()
    } else {
        truncated
    }
}

pub fn unique_download_path(directory: &Path, name: &str) -> PathBuf {
    let name = sanitize_file_name(name);
    let candidate = directory.join(&name);
    if !candidate.exists() {
        return candidate;
    }

    let path = Path::new(&name);
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "File".to_string());
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();

    for index in 1..10_000 {
        let candidate = directory.join(format!("{stem} ({index}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("{stem}-{}{extension}", now_ms()))
}

async fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    match tokio::fs::rename(from, to).await {
        Ok(()) => Ok(()),
        Err(_) => {
            tokio::fs::copy(from, to).await?;
            tokio::fs::remove_file(from).await?;
            Ok(())
        }
    }
}

fn sanitize_reason(reason: &str) -> String {
    let reason: String = reason
        .chars()
        .filter(|character| !character.is_control())
        .take(160)
        .collect();
    if reason.trim().is_empty() {
        "Transfer failed".to_string()
    } else {
        reason
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_path_components() {
        assert_eq!(sanitize_file_name("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_file_name("C:\\temp\\photo.png"), "photo.png");
    }

    #[test]
    fn keeps_unicode_names() {
        assert_eq!(sanitize_file_name("café final.sketch"), "café final.sketch");
    }

    #[test]
    fn avoids_windows_reserved_names() {
        assert_eq!(sanitize_file_name("CON.txt"), "_CON.txt");
    }

    #[test]
    fn limits_long_unicode_names() {
        let name = format!("{}.pdf", "é".repeat(120));
        let sanitized = sanitize_file_name(&name);
        assert!(sanitized.len() <= 200);
        assert!(sanitized.ends_with(".pdf"));
    }
}
