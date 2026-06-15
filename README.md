# efact-hardware-agent

Agente local de hardware para [eFact](https://github.com/nubitio). Conecta el POS web con periféricos que el navegador no puede acceder directamente:

- **Impresoras térmicas** ESC/POS (USB HID o spooler del sistema)
- **Balanzas RS-232** (stream continuo, protocolos comerciales)

## Arquitectura

```
eFact Web POS  →  localhost:8765  →  efact-hardware-agent  →  impresora / puerto serie
```

El agente corre en segundo plano en la máquina del cajero. El POS lo detecta vía `print_method = LOCAL_AGENT` y consulta peso con `GET /scale/weight`.

## Instalación

### Linux / macOS

```bash
curl -fsSL https://raw.githubusercontent.com/nubitio/efact-hardware-agent/main/install.sh | bash
```

### Windows (PowerShell)

```powershell
iwr -useb https://raw.githubusercontent.com/nubitio/efact-hardware-agent/main/install.ps1 | iex
```

El instalador descarga el binario, escribe `config.toml`, registra autostart y en macOS usa `ActivationPolicy::Accessory` para **no mostrar icono en el Dock**.

## API

### Impresión

| Método | Endpoint    | Descripción                    |
|--------|-------------|--------------------------------|
| GET    | `/health`   | Estado del agente y servicios  |
| GET    | `/printers` | Impresoras HID y del sistema   |
| POST   | `/print`    | Bytes ESC/POS (`octet-stream`) |

### Balanza

| Método | Endpoint            | Descripción                              |
|--------|---------------------|------------------------------------------|
| GET    | `/scale/protocols`  | Protocolos soportados                    |
| GET    | `/scale/ports`      | Puertos serie disponibles                |
| GET    | `/scale/status`     | Conexión, protocolo, último error        |
| GET    | `/scale/weight`     | Peso actual `{ kg, stable, raw, … }`     |

`GET /scale/weight` devuelve `stable: true` cuando el peso se mantiene estable el número de lecturas configurado en `stable_reads`.

## Configuración

Ubicaciones (en orden):

1. Junto al binario (`config.toml`)
2. `~/.config/efact-hardware-agent/config.toml` (Linux/macOS)
3. `%APPDATA%\efact-hardware-agent\config.toml` (Windows)

La ruta legada `efact-printer-agent` sigue leyéndose para upgrades.

```toml
port = 8765

# Impresora (campos planos, compatibles con configs anteriores)
# usb_vendor_id = "04b8"
# system_printer_name = "POS_D_BASIC_230"

[scale]
enabled = true
serial_port = "COM3"              # Windows
# serial_port = "/dev/ttyUSB0"    # Linux
# serial_port = "/dev/cu.usbserial-1410"  # macOS
protocol = "excell"
baud_rate = 9600
data_bits = 8
parity = "none"
stop_bits = 1
stable_reads = 3
stable_window_ms = 200
```

### Protocolos de balanza

| ID | Marcas / uso |
|----|----------------|
| `excell` | **Excell** (default, stream continuo ASCII — común en Perú) |
| `generic` | Cualquier balanza que envíe un número con unidad |
| `cas` | CAS CI / LP / ER |
| `toledo` | Mettler Toledo Prix, PS, Tiger |
| `toledo_stx` | Toledo con tramas STX…ETX |
| `mettler_sics` | Mettler MT-SICS |
| `dibal` | Dibal G/M/L (retail) |
| `kretz` | Kretz ARS / eKO (LATAM) |
| `magellan` | Magellan / Datalogic |
| `avery` | Avery Berkel |
| `rahul` | Formato fijo `+00000.000kg` (variante china) |

Lista completa: `GET http://localhost:8765/scale/protocols`

### Configurar balanza Excell (sin modelo exacto)

1. Conectar adaptador USB ↔ RS-232
2. `GET /scale/ports` para ver el puerto
3. Dejar `protocol = "excell"` y `baud_rate = 9600`
4. Si el peso no parsea, probar `generic` o capturar una línea cruda desde `raw` en `/scale/weight`

## Conflicto de puerto 8765 (Docker / dev)

El agente escucha en `127.0.0.1:8765`. Si Docker o el stack de desarrollo de eFact publica otro servicio en `*:8765`, las peticiones a `http://localhost:8765` pueden ir a Symfony (404) en lugar del agente.

**Solución:** el POS usa `http://127.0.0.1:8765`. Para probar manualmente:

```bash
curl http://127.0.0.1:8765/health
```

Si necesitas liberar el puerto, revisa `docker compose ps` y el mapeo `8765:8765`.

## Compilar

```bash
cargo build --release
# binario en target/release/efact-hardware-agent
```

**Requisitos:** Rust 1.75+, `libudev-dev` en Linux.

## Configuración

Puedes configurar el agente de dos formas:

1. **POS web** — botón *Hardware local* en el header del POS (`GET/PUT /config`)
2. **Bandeja del sistema** — menú → *Abrir configuración* (abre la carpeta con `config.toml`)

Un panel nativo de configuración en Rust (ventana propia del agente) está planificado; por ahora el POS web cubre impresora, balanza y guía de cableado.

## Migración desde efact-printer-agent

- Binario renombrado a `efact-hardware-agent`
- El instalador crea symlink `efact-printer-agent` → nuevo binario
- Configs legadas en `~/.config/efact-printer-agent/` siguen funcionando
- Misma URL: `http://localhost:8765`

## Licencia

MIT