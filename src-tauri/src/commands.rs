use crate::config::DeviceInfo;
use crate::models::{Bootstrap, ChatMessage, OutgoingRequest, Peer, now_ms};
use crate::protocol::Control;
use crate::session::{self, Session};
use crate::state::{SharedState, emit};
use crate::transfer::{self, Transfer};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

#[tauri::command]
pub fn get_bootstrap(app: AppHandle, state: State<'_, SharedState>) -> Result<Bootstrap, String> {
    Ok(Bootstrap {
        device: state.device_info(),
        peers: state.peers_snapshot(),
        session: state.session_snapshot(),
        network: state.network_info(),
        discoverable: state.is_discoverable(),
        download_dir: state.downloads_dir.to_string_lossy().to_string(),
        version: app.package_info().version.to_string(),
    })
}

#[tauri::command]
pub fn rename_device(state: State<'_, SharedState>, name: String) -> Result<DeviceInfo, String> {
    state.set_name(&name)
}

#[tauri::command]
pub fn set_discoverable(state: State<'_, SharedState>, discoverable: bool) -> Result<bool, String> {
    state.set_discoverable(discoverable)
}

#[tauri::command]
pub async fn connect_to_peer(
    app: AppHandle,
    state: State<'_, SharedState>,
    peer_id: String,
) -> Result<(), String> {
    let state = state.inner().clone();

    if state.has_session() {
        return Err("A session is already active".to_string());
    }
    if !state.pending_incoming.lock().unwrap().is_empty() {
        return Err("Respond to the pending connection request first".to_string());
    }
    if !state.pending_outgoing.lock().unwrap().is_empty() {
        return Err("Waiting for the other device to approve".to_string());
    }

    let peer: Peer = state
        .peers
        .read()
        .unwrap()
        .get(&peer_id)
        .cloned()
        .ok_or("This device is offline")?;

    if peer.busy {
        return Err("The other device is already in a session".to_string());
    }

    let request_id = Uuid::new_v4().to_string();
    let (task, start) =
        session::spawn_outgoing(app.clone(), state.clone(), request_id.clone(), peer.clone());
    state.pending_outgoing.lock().unwrap().insert(
        request_id.clone(),
        crate::state::PendingOutgoing {
            peer: peer.clone(),
            task,
        },
    );

    emit(
        &app,
        "outgoing-request",
        OutgoingRequest {
            request_id,
            peer,
            status: "pending".to_string(),
            reason: None,
        },
    );
    let _ = start.send(());
    Ok(())
}

#[tauri::command]
pub fn cancel_outgoing(
    app: AppHandle,
    state: State<'_, SharedState>,
    request_id: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    let pending = state.pending_outgoing.lock().unwrap().remove(&request_id);
    let Some(pending) = pending else {
        return Ok(());
    };
    pending.task.abort();
    emit(
        &app,
        "outgoing-request-updated",
        OutgoingRequest {
            request_id,
            peer: pending.peer,
            status: "cancelled".to_string(),
            reason: None,
        },
    );
    Ok(())
}

#[tauri::command]
pub fn respond_connection(
    state: State<'_, SharedState>,
    request_id: String,
    accept: bool,
) -> Result<(), String> {
    let pending = state
        .pending_incoming
        .lock()
        .unwrap()
        .remove(&request_id)
        .ok_or("This connection request has expired")?;
    pending
        .responder
        .send(accept)
        .map_err(|_| "This connection request has expired".to_string())
}

#[tauri::command]
pub async fn disconnect_session(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let state = state.inner().clone();
    let session_id = session::active_session(&state).map(|session| session.id.clone());
    let Some(session_id) = session_id else {
        return Ok(());
    };
    session::terminate_session(&app, &state, &session_id, "local", None).await;
    Ok(())
}

#[tauri::command]
pub async fn send_text(
    app: AppHandle,
    state: State<'_, SharedState>,
    session_id: String,
    text: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    let session = get_session(&state, &session_id)?;

    if text.trim().is_empty() {
        return Err("Message can't be empty".to_string());
    }
    if text.len() > 64 * 1024 {
        return Err("This message is too long".to_string());
    }

    let id = Uuid::new_v4().to_string();
    let at = now_ms();
    session
        .send_control(&Control::Text {
            id: id.clone(),
            text: text.clone(),
            at,
        })
        .await?;

    emit(
        &app,
        "message-added",
        ChatMessage {
            id,
            session_id,
            direction: "out".to_string(),
            kind: "text".to_string(),
            text: Some(text),
            transfer: None,
            at,
        },
    );
    Ok(())
}

#[tauri::command]
pub async fn send_files(
    app: AppHandle,
    state: State<'_, SharedState>,
    session_id: String,
    paths: Vec<String>,
) -> Result<Vec<String>, String> {
    let state = state.inner().clone();
    let session = get_session(&state, &session_id)?;

    if paths.is_empty() {
        return Err("No files selected".to_string());
    }
    if paths.len() > 32 {
        return Err("You can send up to 32 files at a time".to_string());
    }

    let mut files: Vec<(String, u64, PathBuf)> = Vec::new();
    for raw in &paths {
        let path = PathBuf::from(raw);
        let metadata = std::fs::metadata(&path)
            .map_err(|_| format!("Couldn't read file: {}", path.display()))?;
        if !metadata.is_file() {
            return Err("Folders aren't supported yet".to_string());
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled file".to_string());
        files.push((transfer::sanitize_file_name(&name), metadata.len(), path));
    }

    let mut ids = Vec::with_capacity(files.len());
    for (name, size, path) in files {
        let (transfer, message) = transfer::prepare_outgoing(&session, name, size, path);
        transfer::register(&state, &session, transfer.clone());
        emit(&app, "message-added", message);
        ids.push(transfer.id.clone());

        let app = app.clone();
        let session = session.clone();
        tauri::async_runtime::spawn(async move {
            transfer::outgoing_file_task(app, session, transfer).await;
        });
    }

    Ok(ids)
}

#[tauri::command]
pub async fn pause_transfer(
    app: AppHandle,
    state: State<'_, SharedState>,
    transfer_id: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    let (session, transfer) = get_transfer(&state, &transfer_id)?;
    transfer::pause(&app, &session, &transfer).await
}

#[tauri::command]
pub async fn resume_transfer(
    app: AppHandle,
    state: State<'_, SharedState>,
    transfer_id: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    let (session, transfer) = get_transfer(&state, &transfer_id)?;
    transfer::resume(&app, &session, &transfer).await
}

#[tauri::command]
pub async fn cancel_transfer(
    app: AppHandle,
    state: State<'_, SharedState>,
    transfer_id: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    let (session, transfer) = get_transfer(&state, &transfer_id)?;
    transfer::cancel(&app, &session, &transfer).await
}

#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .open_path(&path, None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reveal_path(app: AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|error| error.to_string())
}

fn get_session(state: &SharedState, id: &str) -> Result<Arc<Session>, String> {
    let session = state
        .sessions
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or("The session has ended")?;
    if session.closed.load(Ordering::SeqCst) {
        return Err("The session has ended".to_string());
    }
    Ok(session)
}

fn get_transfer(
    state: &SharedState,
    transfer_id: &str,
) -> Result<(Arc<Session>, Arc<Transfer>), String> {
    let transfer = state
        .transfers
        .lock()
        .unwrap()
        .get(transfer_id)
        .cloned()
        .ok_or("Transfer not found")?;
    let session = get_session(state, &transfer.session_id)?;
    Ok((session, transfer))
}
