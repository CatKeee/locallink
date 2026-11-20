use crate::models::{NetworkInfo, Peer, now_ms};
use crate::state::{SharedState, emit};
use serde::{Deserialize, Serialize};
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::time::sleep;

pub const DISCOVERY_PORT: u16 = 47821;
const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 76, 67);
const ANNOUNCE_INTERVAL: Duration = Duration::from_millis(2500);
const PEER_TIMEOUT_MS: i64 = 8_000;
const MAX_PEERS: usize = 128;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Announcement {
    v: u32,
    kind: String,
    id: String,
    name: String,
    platform: String,
    port: u16,
    busy: bool,
}

pub fn bind() -> io::Result<std::net::UdpSocket> {
    let socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT))?;
    socket.set_broadcast(true)?;
    let _ = socket.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED);
    let _ = socket.set_multicast_loop_v4(false);
    socket.set_nonblocking(true)?;
    Ok(socket)
}

pub fn start(app: tauri::AppHandle, state: SharedState, socket: std::net::UdpSocket) {
    tauri::async_runtime::spawn(async move {
        let socket = match UdpSocket::from_std(socket) {
            Ok(socket) => Arc::new(socket),
            Err(error) => {
                eprintln!("[localink] failed to start discovery socket: {error}");
                return;
            }
        };

        tauri::async_runtime::spawn(receive_loop(app.clone(), state.clone(), socket.clone()));
        tauri::async_runtime::spawn(announce_loop(app, state, socket));
    });
}

async fn receive_loop(app: tauri::AppHandle, state: SharedState, socket: Arc<UdpSocket>) {
    let mut buffer = vec![0u8; 2048];
    loop {
        match socket.recv_from(&mut buffer).await {
            Ok((size, source)) => {
                if let Some(announcement) = parse_announcement(&buffer[..size]) {
                    handle_announcement(&app, &state, &socket, announcement, source).await;
                }
            }
            Err(error) => {
                if error.kind() != io::ErrorKind::WouldBlock {
                    eprintln!("[localink] discovery receive error: {error}");
                    sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }
}

async fn announce_loop(app: tauri::AppHandle, state: SharedState, socket: Arc<UdpSocket>) {
    let mut last_ip_check = Instant::now() - Duration::from_secs(10);
    let mut was_visible = false;

    loop {
        let visible = state.is_discoverable();
        if visible || was_visible {
            let kind = if visible { "announce" } else { "bye" };
            let payload = announcement_bytes(&state, kind);
            let mut targets = vec![
                SocketAddr::from((Ipv4Addr::BROADCAST, DISCOVERY_PORT)),
                SocketAddr::from((MULTICAST_ADDR, DISCOVERY_PORT)),
            ];
            targets.extend(
                crate::state::interface_broadcasts()
                    .into_iter()
                    .map(|address| SocketAddr::from((address, DISCOVERY_PORT))),
            );
            targets.sort();
            targets.dedup();

            for target in targets {
                let _ = socket.send_to(&payload, target).await;
            }
        }
        was_visible = visible;

        prune_peers(&app, &state);

        if last_ip_check.elapsed() >= Duration::from_secs(5) {
            last_ip_check = Instant::now();
            if state.refresh_local_ip() {
                let network: NetworkInfo = state.network_info();
                emit(&app, "network-updated", network);
            }
        }

        tokio::select! {
            _ = sleep(ANNOUNCE_INTERVAL) => {}
            _ = state.announce_notify.notified() => {}
        }
    }
}

async fn handle_announcement(
    app: &tauri::AppHandle,
    state: &SharedState,
    socket: &Arc<UdpSocket>,
    announcement: Announcement,
    source: SocketAddr,
) {
    let self_id = state.device_info().id;
    if announcement.id == self_id {
        return;
    }

    if announcement.kind == "bye" {
        let removed = state
            .peers
            .write()
            .unwrap()
            .remove(&announcement.id)
            .is_some();
        if removed {
            emit(app, "peers-updated", state.peers_snapshot());
        }
        return;
    }

    if announcement.port == 0 {
        return;
    }

    let ip = source.ip().to_string();
    let (is_new, changed) = {
        let mut peers = state.peers.write().unwrap();
        if !peers.contains_key(&announcement.id) && peers.len() >= MAX_PEERS {
            return;
        }

        let is_new = !peers.contains_key(&announcement.id);
        let changed = match peers.get(&announcement.id) {
            Some(peer) => {
                peer.name != announcement.name
                    || peer.platform != announcement.platform
                    || peer.ip != ip
                    || peer.port != announcement.port
                    || peer.busy != announcement.busy
            }
            None => true,
        };

        if changed || is_new {
            peers.insert(
                announcement.id.clone(),
                Peer {
                    id: announcement.id.clone(),
                    name: announcement.name.clone(),
                    platform: announcement.platform.clone(),
                    ip: ip.clone(),
                    port: announcement.port,
                    busy: announcement.busy,
                    last_seen: now_ms(),
                },
            );
        } else if let Some(peer) = peers.get_mut(&announcement.id) {
            peer.last_seen = now_ms();
        }
        (is_new, changed)
    };

    if changed {
        emit(app, "peers-updated", state.peers_snapshot());
    }

    // Answer a newly discovered device immediately, so both sides populate
    // their lists without waiting for the next broadcast tick. Never answer
    // while invisible: scanning should not leak our presence.
    if is_new && state.is_discoverable() {
        let payload = announcement_bytes(state, "announce");
        let _ = socket
            .send_to(&payload, SocketAddr::new(source.ip(), source.port()))
            .await;
    }
}

fn prune_peers(app: &tauri::AppHandle, state: &SharedState) {
    let now = now_ms();
    let removed = {
        let mut peers = state.peers.write().unwrap();
        let before = peers.len();
        peers.retain(|_, peer| now - peer.last_seen <= PEER_TIMEOUT_MS);
        peers.len() != before
    };
    if removed {
        emit(app, "peers-updated", state.peers_snapshot());
    }
}

fn announcement_bytes(state: &SharedState, kind: &str) -> Vec<u8> {
    let device = state.device_info();
    let announcement = Announcement {
        v: crate::protocol::PROTOCOL_VERSION,
        kind: kind.to_string(),
        id: device.id,
        name: device.name,
        platform: device.platform,
        port: state.tcp_port,
        busy: state.busy.load(std::sync::atomic::Ordering::Relaxed),
    };
    serde_json::to_vec(&announcement).unwrap_or_default()
}

fn parse_announcement(payload: &[u8]) -> Option<Announcement> {
    let mut announcement: Announcement = serde_json::from_slice(payload).ok()?;
    if announcement.v != crate::protocol::PROTOCOL_VERSION
        || !matches!(announcement.kind.as_str(), "announce" | "bye")
    {
        return None;
    }
    if announcement.id.len() > 64
        || announcement.name.len() > 64
        || announcement.platform.len() > 16
    {
        return None;
    }
    announcement.name = crate::config::clean_name(&announcement.name)?;
    if announcement.id.is_empty() {
        return None;
    }
    Some(announcement)
}
