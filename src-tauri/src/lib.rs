mod commands;
mod config;
mod discovery;
mod models;
mod protocol;
mod session;
mod state;
mod transfer;

use state::SharedState;
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();

            let tcp_listener = std::net::TcpListener::bind(("0.0.0.0", 0))?;
            let tcp_port = tcp_listener.local_addr()?.port();

            let discovery_socket = match discovery::bind() {
                Ok(socket) => Some(socket),
                Err(error) => {
                    eprintln!("[localink] discovery is unavailable: {error}");
                    None
                }
            };

            let state: SharedState = Arc::new(state::AppState::new(
                &handle,
                tcp_port,
                discovery_socket.is_some(),
            )?);

            app.manage(state.clone());
            session::clean_cache(&state);
            session::start_tcp_server(handle.clone(), state.clone(), tcp_listener);

            if let Some(socket) = discovery_socket {
                discovery::start(handle.clone(), state.clone(), socket);
            }
            transfer::start_progress_ticker(handle.clone(), state.clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap,
            commands::rename_device,
            commands::set_discoverable,
            commands::connect_to_peer,
            commands::cancel_outgoing,
            commands::respond_connection,
            commands::disconnect_session,
            commands::send_text,
            commands::send_files,
            commands::pause_transfer,
            commands::resume_transfer,
            commands::cancel_transfer,
            commands::open_path,
            commands::reveal_path,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Localink");

    app.run(|handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            let state = handle.state::<SharedState>();
            state.clear_cache();
        }
    });
}
