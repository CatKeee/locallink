# Localink

Localink is a lightweight local-network transfer tool for macOS and Windows. Send files or a quick piece of text directly between two computers — no account, no cloud, and no chat app required.

Open the app and nearby devices appear automatically. Approve a connection, then exchange messages and files in a simple chat-like session.

> Built for trusted local networks. Transfers use direct peer-to-peer TCP and are not additionally encrypted.

## Download

Prebuilt macOS builds for Apple Silicon are available on the [Releases page](https://github.com/CatKeee/locallink/releases). Windows users can build from source.

> The current macOS build is ad-hoc signed and not notarized. On first launch, right-click Localink and choose **Open**. See the release notes for details.

## Features

- **Discover on launch**: no modes to choose. Localink opens directly to nearby devices; discovery runs while the app is open.
- **Discoverable switch**: hide from nearby devices while still being able to browse and connect to them. Hidden devices reject incoming requests.
- **Connection approval**: every session must be accepted by the other person.
- **Chat-style transfer**: text messages and files can be sent in the same conversation.
- **Large files**: chunked, streamed transfers keep memory use low, even for multi-gigabyte files.
- **Live progress**: percentage, speed, transferred size, and estimated time remaining.
- **Pause, resume, cancel**: resume from the current offset instead of starting over.
- **Ephemeral by design**: messages and unfinished transfers are cleared when you disconnect. Files you have already received are kept.
- **Device naming**: uses the computer name by default and can be renamed at any time.
- **Cross-platform**: Apple Silicon macOS and Windows 10/11.
- **Small and private**: no sign-in, no background daemon, no telemetry.

## Usage

1. Open Localink on both computers and connect them to the same local network.
2. Localink searches automatically; select the other device under **Available devices**.
3. The other person approves the connection request.
4. Send text with Enter, or use the paperclip / drag files into the window.

If you only want to look around without being visible, turn off **Discoverable** in the local device bar. You can still see nearby devices and initiate connections; incoming requests will be rejected.

Received files are saved to:

- macOS: `~/Downloads/Localink`
- Windows: `%USERPROFILE%\Downloads\Localink`

Disconnecting clears the session history and incomplete transfers. Completed files remain on disk.

## Tech Stack

- **Desktop shell**: Tauri 2 (Rust)
- **Frontend**: React 19 + TypeScript + Vite
- **Styling**: Tailwind CSS 4
- **Components**: shadcn/ui (Base UI, `base-nova` style)
- **Icons**: Lucide
- **Package manager**: pnpm

## How It Works

### Discovery

- A small JSON announcement is sent over UDP to the broadcast address, a multicast group, and each active interface's subnet broadcast address.
- Peers record the device ID, name, platform, TCP port, and busy state. A peer that has not announced for 8 seconds is removed.
- Newly discovered devices receive an immediate unicast reply so both lists fill quickly.
- Turning **Discoverable** off broadcasts a `bye` announcement so peers remove the device immediately. Scanning continues so the hidden device can still initiate connections.
- Discovery runs only while the app is open.

### Sessions

- Each connection begins with a protocol handshake. The receiving side decides whether to accept, and an unanswered request expires after 45 seconds.
- Only one session is active at a time. Busy state is shared through discovery announcements.
- Control messages and file chunks share one TCP connection. A length-prefixed frame format separates JSON control messages from binary chunks.
- The send queue is bounded, giving natural TCP backpressure and keeping memory use to a few megabytes.

### File Transfer

- Files are sent in 256 KiB chunks. The receiver writes to a `.part` file in the app cache directory.
- Once the size is verified, the file is moved to the Downloads folder. Duplicate names are handled automatically.
- Progress is pushed roughly four times per second rather than once per chunk.
- Pause and resume are synchronized over control frames; either side can request a pause.
- Cancelling, disconnecting, or quitting removes incomplete cache files. Startup also cleans any leftovers.
- File names are sanitized for both macOS and Windows, including duplicate names and Windows reserved names.
- Available disk space is checked before accepting a file.

### Lightweight by Default

- No database: messages and transfer state live in memory.
- The UI caps message history to keep long sessions small.
- Partial files live in the system cache and move out after completion.
- Release builds use LTO, `codegen-units = 1`, and symbol stripping.

## Development

### Requirements

- Node.js 20.19+ (or 22.12+)
- pnpm
- Rust stable and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform

### Run

```bash
git clone git@github.com:CatKeee/locallink.git
cd locallink
pnpm install
pnpm tauri dev
```

Frontend-only development:

```bash
pnpm dev
```

### Check and build

```bash
pnpm build
cd src-tauri && cargo check
pnpm tauri build
```

## Project Structure

```text
.
├── src/
│   ├── components/          # App components and shadcn/ui primitives
│   ├── hooks/               # Tauri event bridge and state
│   ├── lib/                 # Formatting and utility helpers
│   ├── views/               # Nearby devices and chat
│   ├── App.tsx
│   └── types.ts
├── src-tauri/
│   ├── src/
│   │   ├── discovery.rs     # UDP discovery
│   │   ├── protocol.rs      # Frame and control protocol
│   │   ├── session.rs       # Handshake, session, lifecycle
│   │   ├── transfer.rs      # Chunking, progress, resume, cleanup
│   │   ├── state.rs         # App state and paths
│   │   └── commands.rs      # Frontend commands
│   ├── capabilities/
│   └── tauri.conf.json
└── components.json
```

## Before Publishing

- Change `productName` and `identifier` in `src-tauri/tauri.conf.json`.
- Replace `src-tauri/icons/app-icon.svg` and run `pnpm tauri icon src-tauri/icons/app-icon.svg` to regenerate platform icons.
- macOS notarization and Windows code signing require your own certificates. See the Tauri distribution documentation.

## Network Notes

- **macOS**: allow Localink to access the local network when prompted.
- **Windows**: allow Localink through Windows Defender Firewall on private or public networks as appropriate.
- Both devices must be on the same subnet, and the router must not have AP/client isolation enabled.
- Only one Localink instance should run per computer; the discovery port is fixed at `47821`.
- If discovery cannot start, the app reports it and suggests closing another instance or checking the firewall.

## Known Limitations

- File transfer is IPv4-only and limited to the local network. No cross-subnet or public-internet transfers.
- One active session at a time.
- Folders are not supported yet; up to 32 files per batch.
- Resume works within a session. Session state is intentionally not restored after quitting.
- TCP transfers are not additionally encrypted; use on trusted networks.
- The macOS builds target Apple Silicon. Intel Macs are not currently supported.

## Roadmap

- Folder transfers and a multi-file queue
- Optional end-to-end encryption
- Manual IP connection and QR pairing
- Clipboard shortcut and global hotkey
- Completion notifications and sounds

## License

[MIT](./LICENSE)
