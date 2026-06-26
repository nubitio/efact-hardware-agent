# efact-hardware-agent

Local hardware agent for [eFact](https://github.com/nubitio). Bridges the web POS to peripherals the browser cannot access directly:

- **Thermal printers** — ESC/POS via USB HID or the OS print spooler
- **RS-232 scales** — continuous weight stream with multiple vendor parsers

## Architecture

```
eFact Web POS  →  127.0.0.1:8765  →  efact-hardware-agent  →  printer / serial port
```

The agent runs in the background on the cashier machine. The POS uses `print_method = LOCAL_AGENT` and reads weight from `GET /scale/weight`.

## Install

### Linux / macOS

```bash
curl -fsSL https://raw.githubusercontent.com/nubitio/efact-hardware-agent/main/install.sh | bash
```

### Windows (PowerShell)

```powershell
iwr -useb https://raw.githubusercontent.com/nubitio/efact-hardware-agent/main/install.ps1 | iex
```

The installer downloads the binary, writes `config.toml`, copies `config.toml.example` (reference, never overwritten on save), and registers autostart. On macOS the agent uses `ActivationPolicy::Accessory` so **no Dock icon** appears.

## API

### Printer

| Method | Endpoint    | Description              |
|--------|-------------|--------------------------|
| GET    | `/health`   | Agent and service status |
| GET    | `/config`   | Current configuration    |
| PUT    | `/config`   | Update printer / scale   |
| GET    | `/printers` | HID and system printers  |
| POST   | `/print`    | ESC/POS bytes (`application/octet-stream`) |

### Scale

| Method | Endpoint           | Description                        |
|--------|--------------------|------------------------------------|
| GET    | `/scale/protocols` | Supported protocol IDs             |
| GET    | `/scale/ports`     | Available serial ports             |
| GET    | `/scale/status`    | Connection, protocol, last error   |
| GET    | `/scale/weight`    | `{ kg, stable, raw, … }`           |

`stable: true` when the weight matches for `stable_reads` consecutive samples within `stable_window_ms`.

## Configuration

**Live file:** `config.toml` — edited by the POS or the agent; comments are stripped on save.

**Reference:** `config.toml.example` — English commented template; safe to keep open while editing; refreshed on reinstall.

Search order:

1. `config.toml` next to the binary
2. `~/.config/efact-hardware-agent/config.toml` (Linux / macOS)
3. `%APPDATA%\efact-hardware-agent\config.toml` (Windows)

Legacy `efact-printer-agent` paths are still read for upgrades.

### End users (POS)

Open **Hardware local** in the POS header — printer spooler toggle, scale port, live weight. Basic settings persist automatically.

### Technicians (advanced)

1. Tray icon → **Abrir configuración** (opens the config folder)
2. Read **`config.toml.example`** for field reference and protocol IDs
3. Edit **`config.toml`** for protocol, baud rate, USB vendor IDs, etc.
4. Diagnose with:

```bash
curl http://127.0.0.1:8765/health
curl http://127.0.0.1:8765/scale/ports
curl http://127.0.0.1:8765/scale/weight   # inspect "raw" when kg is null
```

If `raw` shows garbage, try `baud_rate = 4800` or a vendor-specific `protocol` (see example file).

### Scale protocols

| ID | Typical hardware |
|----|------------------|
| `generic` | **Default** — first decimal in each line |
| `excell` | Excell continuous ASCII (Peru retail) |
| `cas` | CAS CI / LP / ER |
| `toledo` | Mettler Toledo Prix, PS, Tiger |
| `toledo_stx` | Toledo STX…ETX frames |
| `mettler_sics` | Mettler MT-SICS |
| `dibal` | Dibal G/M/L |
| `kretz` | Kretz ARS / eKO (LATAM) |
| `magellan` | Magellan / Datalogic |
| `avery` | Avery Berkel |
| `rahul` | `+00000.000kg` fixed-width variants |

Full list: `GET http://127.0.0.1:8765/scale/protocols`

## Port 8765 conflict (Docker / dev)

The agent listens on **`127.0.0.1:8765` only**. If Docker maps another service to `*:8765`, `http://localhost:8765` may hit Symfony (404) instead of the agent because macOS resolves `localhost` to IPv6 first.

**Fix:** the POS uses `http://127.0.0.1:8765`. Manual check:

```bash
curl http://127.0.0.1:8765/health
```

## Build

```bash
cargo build --release
# binary: target/release/efact-hardware-agent
```

**Requirements:** Rust 1.75+, `libudev-dev` on Linux.

## Migration from efact-printer-agent

- Binary renamed to `efact-hardware-agent`
- Installer keeps a `efact-printer-agent` symlink to the new binary
- Legacy configs under `~/.config/efact-printer-agent/` still work
- Same listen address: `127.0.0.1:8765`

## License

MIT