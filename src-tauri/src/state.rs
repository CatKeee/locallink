use crate::config::{Config, DeviceInfo, detect_default_name, platform};
use crate::models::{NetworkInfo, Peer, SessionInfo};
use crate::session::Session;
use crate::transfer::Transfer;
use if_addrs::{IfAddr, Ifv4Addr};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::net::{Ipv4Addr, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tauri::Manager;
use tokio::sync::{Notify, Semaphore, oneshot};
use uuid::Uuid;

pub type SharedState = Arc<AppState>;

pub struct PendingIncoming {
    pub responder: oneshot::Sender<bool>,
}

pub struct PendingOutgoing {
    pub peer: Peer,
    pub task: tauri::async_runtime::JoinHandle<()>,
}

pub struct AppState {
    pub config_path: PathBuf,
    pub config: RwLock<Config>,
    pub downloads_dir: PathBuf,
    pub transfers_dir: PathBuf,
    pub local_ip: RwLock<String>,
    pub tcp_port: u16,
    pub discovery_available: bool,
    pub peers: RwLock<HashMap<String, Peer>>,
    pub sessions: Mutex<HashMap<String, Arc<Session>>>,
    pub pending_incoming: Mutex<HashMap<String, PendingIncoming>>,
    pub pending_outgoing: Mutex<HashMap<String, PendingOutgoing>>,
    pub transfers: Mutex<HashMap<String, Arc<Transfer>>>,
    pub busy: AtomicBool,
    pub announce_notify: Notify,
    pub handshake_slots: Arc<Semaphore>,
}

impl AppState {
    pub fn new(
        app: &tauri::AppHandle,
        tcp_port: u16,
        discovery_available: bool,
    ) -> io::Result<Self> {
        let config_dir = app
            .path()
            .app_config_dir()
            .map_err(|error| io::Error::other(error.to_string()))?;
        let cache_dir = app
            .path()
            .app_cache_dir()
            .map_err(|error| io::Error::other(error.to_string()))?;
        let download_root = app
            .path()
            .download_dir()
            .or_else(|_| app.path().home_dir().map(|home| home.join("Downloads")))
            .map_err(|error| io::Error::other(error.to_string()))?;

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&cache_dir)?;
        let transfers_dir = cache_dir.join("transfers");
        fs::create_dir_all(&transfers_dir)?;
        let downloads_dir = download_root.join("Localink");
        fs::create_dir_all(&downloads_dir)?;

        let config_path = config_dir.join("config.json");
        let mut config: Config = fs::read_to_string(&config_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();

        let mut needs_save = false;
        if config.id.trim().is_empty() {
            config.id = Uuid::new_v4().to_string();
            needs_save = true;
        }
        if config.name.trim().is_empty() {
            config.name = detect_default_name();
            needs_save = true;
        }
        if needs_save {
            let _ = save_config_to(&config_path, &config);
        }

        Ok(Self {
            config_path,
            config: RwLock::new(config),
            downloads_dir,
            transfers_dir,
            local_ip: RwLock::new(detect_local_ip()),
            tcp_port,
            discovery_available,
            peers: RwLock::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            pending_incoming: Mutex::new(HashMap::new()),
            pending_outgoing: Mutex::new(HashMap::new()),
            transfers: Mutex::new(HashMap::new()),
            busy: AtomicBool::new(false),
            announce_notify: Notify::new(),
            handshake_slots: Arc::new(Semaphore::new(8)),
        })
    }

    pub fn device_info(&self) -> DeviceInfo {
        let config = self.config.read().unwrap();
        DeviceInfo {
            id: config.id.clone(),
            name: config.name.clone(),
            platform: platform().to_string(),
        }
    }

    pub fn set_name(&self, raw: &str) -> Result<DeviceInfo, String> {
        let name = crate::config::clean_name(raw).ok_or("Device name can't be empty")?;
        {
            let mut config = self.config.write().unwrap();
            config.name = name;
            save_config_to(&self.config_path, &config).map_err(|error| error.to_string())?;
        }
        self.announce_notify.notify_one();
        Ok(self.device_info())
    }

    pub fn is_discoverable(&self) -> bool {
        self.config.read().unwrap().discoverable
    }

    pub fn set_discoverable(&self, discoverable: bool) -> Result<bool, String> {
        {
            let mut config = self.config.write().unwrap();
            config.discoverable = discoverable;
            save_config_to(&self.config_path, &config).map_err(|error| error.to_string())?;
        }
        self.announce_notify.notify_one();
        Ok(discoverable)
    }

    pub fn peers_snapshot(&self) -> Vec<Peer> {
        self.peers.read().unwrap().values().cloned().collect()
    }

    pub fn session_snapshot(&self) -> Option<SessionInfo> {
        self.sessions
            .lock()
            .unwrap()
            .values()
            .next()
            .map(|session| session.info())
    }

    pub fn network_info(&self) -> NetworkInfo {
        NetworkInfo {
            ip: self.local_ip.read().unwrap().clone(),
            discovery_port: crate::discovery::DISCOVERY_PORT,
            available: self.discovery_available && self.tcp_port != 0,
        }
    }

    pub fn has_session(&self) -> bool {
        !self.sessions.lock().unwrap().is_empty()
    }

    pub fn refresh_local_ip(&self) -> bool {
        let ip = detect_local_ip();
        let mut current = self.local_ip.write().unwrap();
        if *current != ip {
            *current = ip;
            true
        } else {
            false
        }
    }

    pub fn set_busy(&self, busy: bool) {
        if self.busy.swap(busy, Ordering::SeqCst) != busy {
            self.announce_notify.notify_one();
        }
    }

    pub fn clear_cache(&self) {
        let Ok(entries) = fs::read_dir(&self.transfers_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let _ = fs::remove_file(path);
            }
        }
    }
}

pub fn save_config_to(path: &PathBuf, config: &Config) -> io::Result<()> {
    let raw = serde_json::to_vec_pretty(config)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, raw)?;
    // Windows does not replace an existing file with rename.
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(tmp, path)
}

pub fn emit<T: Serialize + Clone>(app: &tauri::AppHandle, event: &str, payload: T) {
    use tauri::Emitter;
    let _ = app.emit(event, payload);
}

pub fn lan_interfaces() -> Vec<Ifv4Addr> {
    let Ok(interfaces) = if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    let mut addresses: Vec<Ifv4Addr> = interfaces
        .into_iter()
        .filter(|interface| !interface.is_p2p)
        .filter_map(|interface| match interface.addr {
            IfAddr::V4(address) => Some(address),
            IfAddr::V6(_) => None,
        })
        .filter(|address| {
            !address.is_loopback()
                && !address.ip.is_unspecified()
                && address.ip != Ipv4Addr::BROADCAST
        })
        .collect();
    addresses.sort_by_key(|address| address.ip);
    addresses.dedup_by_key(|address| address.ip);
    addresses
}

pub fn detect_local_ip() -> String {
    // Prefer real private-LAN interfaces. VPN clients often install a
    // 198.18.0.0/15 or point-to-point route, which must not be advertised as
    // the local address for file transfer.
    let mut candidates = lan_interfaces();
    candidates.sort_by_key(|address| {
        let octets = address.ip.octets();
        match octets {
            [192, 168, ..] => 0,
            [10, ..] => 1,
            [172, second, ..] if (16..=31).contains(&second) => 2,
            [100, second, ..] if (64..=127).contains(&second) => 3,
            [169, 254, ..] => 5,
            [198, 18, ..] | [198, 19, ..] => 10,
            _ => 4,
        }
    });
    if let Some(address) = candidates.first() {
        return address.ip.to_string();
    }

    for target in [
        ("8.8.8.8", 80),
        ("239.255.76.67", crate::discovery::DISCOVERY_PORT),
        ("255.255.255.255", 9),
    ] {
        let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) else {
            continue;
        };
        if socket.connect(target).is_err() {
            continue;
        }
        let Ok(addr) = socket.local_addr() else {
            continue;
        };
        let ip = addr.ip();
        if !ip.is_unspecified() && !ip.is_loopback() {
            return ip.to_string();
        }
    }
    "0.0.0.0".to_string()
}

pub fn interface_broadcasts() -> Vec<Ipv4Addr> {
    let mut targets: Vec<Ipv4Addr> = lan_interfaces()
        .into_iter()
        .filter_map(|address| {
            address.broadcast.or_else(|| {
                let ip = u32::from(address.ip);
                let mask = u32::from(address.netmask);
                if mask == 0 {
                    None
                } else {
                    Some(Ipv4Addr::from(ip | !mask))
                }
            })
        })
        .filter(|broadcast| {
            *broadcast != Ipv4Addr::UNSPECIFIED && *broadcast != Ipv4Addr::BROADCAST
        })
        .collect();
    targets.sort();
    targets.dedup();
    targets
}
