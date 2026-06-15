#!/usr/bin/env bash
# efact-hardware-agent installer — Linux & macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/nubitio/efact-hardware-agent/main/install.sh | bash
set -euo pipefail

REPO="nubitio/efact-hardware-agent"
BINARY="efact-hardware-agent"
LEGACY_BINARY="efact-printer-agent"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="${HOME}/.config/efact-hardware-agent"
LEGACY_CONFIG_DIR="${HOME}/.config/efact-printer-agent"

info()  { printf '\033[0;34m[efact-hardware-agent]\033[0m %s\n' "$*"; }
ok()    { printf '\033[0;32m[efact-hardware-agent]\033[0m %s\n' "$*"; }
err()   { printf '\033[0;31m[efact-hardware-agent]\033[0m %s\n' "$*" >&2; exit 1; }

need() { command -v "$1" &>/dev/null || err "Required tool not found: $1"; }
need curl
need tar

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
  Linux)
    case "${ARCH}" in
      x86_64) ASSET="efact-hardware-agent-linux-x86_64.tar.gz" ;;
      *)      err "Unsupported architecture: ${ARCH}" ;;
    esac
    ;;
  Darwin)
    case "${ARCH}" in
      x86_64)  ASSET="efact-hardware-agent-macos-x86_64.tar.gz" ;;
      arm64)   ASSET="efact-hardware-agent-macos-arm64.tar.gz" ;;
      *)       err "Unsupported architecture: ${ARCH}" ;;
    esac
    ;;
  *) err "Unsupported OS: ${OS}" ;;
esac

if [ "${OS}" = "Linux" ]; then
  if ! ldconfig -p 2>/dev/null | grep -q 'libayatana-appindicator3\|libappindicator3'; then
    info "libappindicator3 not found — attempting to install..."
    if command -v apt-get &>/dev/null; then
      sudo apt-get install -y libayatana-appindicator3-1 2>/dev/null \
        || sudo apt-get install -y libappindicator3-1
    elif command -v dnf &>/dev/null; then
      sudo dnf install -y libappindicator-gtk3
    elif command -v pacman &>/dev/null; then
      sudo pacman -S --noconfirm libappindicator-gtk3
    else
      err "Cannot install libappindicator3 automatically. Please install it manually and re-run this script."
    fi
    ok "libappindicator3 installed."
  fi
fi

info "Fetching latest release..."
TAG="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"

[ -n "${TAG}" ] || err "Could not determine latest release tag."
info "Latest release: ${TAG}"

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
info "Downloading ${ASSET}..."
curl -fsSL "${DOWNLOAD_URL}" -o "${TMP}/${ASSET}"

tar -xzf "${TMP}/${ASSET}" -C "${TMP}"

if [ -w "${INSTALL_DIR}" ]; then
  install -m 755 "${TMP}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
else
  info "Installing to ${INSTALL_DIR} (sudo required)..."
  sudo install -m 755 "${TMP}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
fi

# Keep legacy symlink so existing LaunchAgents/systemd units keep working.
ln -sf "${INSTALL_DIR}/${BINARY}" "${INSTALL_DIR}/${LEGACY_BINARY}" 2>/dev/null || true

if [ ! -f "${CONFIG_DIR}/config.toml" ]; then
  mkdir -p "${CONFIG_DIR}"
  if [ -f "${LEGACY_CONFIG_DIR}/config.toml" ]; then
    cp "${LEGACY_CONFIG_DIR}/config.toml" "${CONFIG_DIR}/config.toml"
    info "Migrated config from ${LEGACY_CONFIG_DIR}/config.toml"
  else
    cp "${TMP}/config.toml" "${CONFIG_DIR}/config.toml"
    info "Default config written to ${CONFIG_DIR}/config.toml"
  fi
fi

setup_systemd() {
  SERVICE_FILE="${HOME}/.config/systemd/user/efact-hardware-agent.service"
  LEGACY_SERVICE="${HOME}/.config/systemd/user/efact-printer-agent.service"
  mkdir -p "$(dirname "${SERVICE_FILE}")"
  cat > "${SERVICE_FILE}" <<EOF
[Unit]
Description=eFact Hardware Agent
After=network.target

[Service]
ExecStart=${INSTALL_DIR}/${BINARY}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF
  systemctl --user disable --now efact-printer-agent 2>/dev/null || true
  [ -f "${LEGACY_SERVICE}" ] && rm -f "${LEGACY_SERVICE}"
  systemctl --user daemon-reload
  systemctl --user enable --now efact-hardware-agent
  ok "systemd user service enabled and started."
}

setup_launchagent() {
  PLIST="${HOME}/Library/LaunchAgents/io.nubit.efact-hardware-agent.plist"
  LEGACY_PLIST="${HOME}/Library/LaunchAgents/io.nubit.efact-printer-agent.plist"
  LABEL="io.nubit.efact-hardware-agent"
  DOMAIN="gui/$(id -u)"
  cat > "${PLIST}" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>${LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>${INSTALL_DIR}/${BINARY}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ProcessType</key>
  <string>Background</string>
  <key>StandardOutPath</key>
  <string>${HOME}/Library/Logs/efact-hardware-agent/agent.log</string>
  <key>StandardErrorPath</key>
  <string>${HOME}/Library/Logs/efact-hardware-agent/agent.log</string>
</dict>
</plist>
EOF

  launchctl bootout "${DOMAIN}"/io.nubit.efact-printer-agent &>/dev/null || true
  [ -f "${LEGACY_PLIST}" ] && rm -f "${LEGACY_PLIST}"
  launchctl bootout "${DOMAIN}"/"${LABEL}" &>/dev/null || true
  launchctl bootstrap "${DOMAIN}" "${PLIST}"
  launchctl kickstart -k "${DOMAIN}"/"${LABEL}"
  ok "LaunchAgent registered and started (sin icono en el Dock)."
}

if [ "${OS}" = "Linux" ] && command -v systemctl &>/dev/null; then
  setup_systemd
elif [ "${OS}" = "Darwin" ]; then
  setup_launchagent
fi

ok "efact-hardware-agent ${TAG} installed successfully."
ok "Agent running on http://localhost:8765"
ok "Config: ${CONFIG_DIR}/config.toml"