#!/bin/sh
set -eu
umask 077

BASE_URL="${XIMONITOR_SERVER_BASE_URL:-https://github.com/XiNian-dada/XiMonitor/releases/latest/download}"
INSTALL_ROOT_DEFAULT="/opt/ximonitor"
LISTEN_HOST_DEFAULT="127.0.0.1"
SERVICE_NAME="ximonitor-server"
BIN_PATH="/usr/local/bin/ximonitor-server"
UNIT_PATH="/etc/systemd/system/${SERVICE_NAME}.service"
MODE="${XIMONITOR_SERVER_MODE:-auto}"

TMP_BIN=""
TMP_SHA256=""

fail() {
  printf '%s\n' "install-server: $*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

cleanup() {
  [ -n "$TMP_BIN" ] && rm -f "$TMP_BIN"
  [ -n "$TMP_SHA256" ] && rm -f "$TMP_SHA256"
}

trap cleanup EXIT HUP INT TERM

clear_screen() {
  if command -v clear >/dev/null 2>&1; then
    clear
    return 0
  fi
  printf '\033c'
}

read_line() {
  prompt="$1"
  default_value="$2"

  [ -r /dev/tty ] || fail "interactive input requires a controlling terminal"
  if [ -n "$default_value" ]; then
    printf '%s [%s]: ' "$prompt" "$default_value" >/dev/tty
  else
    printf '%s: ' "$prompt" >/dev/tty
  fi
  IFS= read -r value </dev/tty || fail "failed to read user input"
  if [ -z "$value" ]; then
    value="$default_value"
  fi
  printf '%s' "$value"
}

prompt_required() {
  prompt="$1"
  default_value="$2"

  while :; do
    value="$(read_line "$prompt" "$default_value")"
    if [ -n "$value" ]; then
      printf '%s' "$value"
      return 0
    fi
    printf '%s\n' "This field is required." >/dev/tty
  done
}

prompt_mode() {
  default_value="$1"

  while :; do
    value="$(read_line "Operation mode (install/upgrade)" "$default_value")"
    case "$value" in
      install|upgrade)
        printf '%s' "$value"
        return 0
        ;;
      *)
        printf '%s\n' "Please enter install or upgrade." >/dev/tty
        ;;
    esac
  done
}

confirm_default_no() {
  prompt="$1"

  while :; do
    answer="$(read_line "$prompt" "n")"
    case "$answer" in
      y|Y|yes|YES)
        return 0
        ;;
      n|N|no|NO)
        return 1
        ;;
      *)
        printf '%s\n' "Please answer y or n." >/dev/tty
        ;;
    esac
  done
}

random_hex() {
  bytes="$1"
  od -An -N"$bytes" -tx1 /dev/urandom | tr -d ' \n'
}

random_port() {
  raw_port="$(od -An -N2 -tu2 /dev/urandom | tr -d ' ')"
  printf '%s' "$((20000 + raw_port % 20000))"
}

calculate_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | sed 's/[[:space:]].*$//'
    return 0
  fi
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | sed 's/[[:space:]].*$//'
    return 0
  fi
  fail "missing required command: sha256sum or shasum"
}

fetch_expected_sha256() {
  artifact_name="$1"
  checksums_url="$BASE_URL/SHA256SUMS.txt"

  printf '%s\n' "Fetching release checksums from $checksums_url"
  curl -fsSL "$checksums_url" -o "$TMP_SHA256" || fail "failed to fetch release checksums"

  expected_sha256="$(awk -v artifact="$artifact_name" '
    NF >= 2 {
      path = $2
      sub(/^\*/, "", path)
      count = split(path, parts, "/")
      if (parts[count] == artifact) {
        print $1
        exit
      }
    }
  ' "$TMP_SHA256")"

  [ -n "$expected_sha256" ] || fail "missing checksum entry for $artifact_name"
  printf '%s' "$expected_sha256"
}

resolve_release_base_url() {
  case "$BASE_URL" in
    https://github.com/*/releases/latest/download)
      releases_root="${BASE_URL%/latest/download}"
      redirect_location="$(curl -fsSI -o /dev/null -w '%header{location}' "$releases_root/latest")" \
        || fail "failed to resolve latest GitHub release"
      [ -n "$redirect_location" ] || fail "GitHub latest release redirect did not include a location"
      resolved_tag="${redirect_location##*/}"
      BASE_URL="${releases_root}/download/${resolved_tag}"
      printf '%s\n' "Resolved GitHub latest release tag: $resolved_tag"
      ;;
  esac
}

validate_port() {
  value="$1"
  case "$value" in
    ''|*[!0-9]*)
      fail "listen port must be a number between 1 and 65535"
      ;;
  esac
  if [ "$value" -lt 1 ] || [ "$value" -gt 65535 ]; then
    fail "listen port must be between 1 and 65535"
  fi
}

validate_scheme() {
  value="$1"
  case "$value" in
    http|https)
      return 0
      ;;
    *)
      fail "public scheme must be http or https"
      ;;
  esac
}

validate_no_whitespace() {
  field="$1"
  value="$2"
  case "$value" in
    *[[:space:]]*)
      fail "$field must not contain whitespace"
      ;;
  esac
}

detect_existing_install_root() {
  if [ -r "$UNIT_PATH" ]; then
    awk -F= '/^WorkingDirectory=/{print $2; exit}' "$UNIT_PATH"
    return 0
  fi

  printf '%s' ""
}

toml_get_raw() {
  file="$1"
  section="$2"
  key="$3"

  awk -v section="[$section]" -v key="$key" '
    /^\[/ {
      in_section = ($0 == section)
      next
    }
    in_section {
      line = $0
      sub(/^[[:space:]]+/, "", line)
      if (line ~ "^" key "[[:space:]]*=") {
        sub(/^[^=]+=[[:space:]]*/, "", line)
        print line
        exit
      }
    }
  ' "$file"
}

trim_whitespace() {
  printf '%s' "$1" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//'
}

strip_toml_string_quotes() {
  value="$(trim_whitespace "$1")"
  case "$value" in
    \"*\")
      value="${value#\"}"
      value="${value%\"}"
      ;;
  esac
  printf '%s' "$value"
}

load_existing_server_defaults() {
  [ -f "$CONFIG_PATH" ] || return 0

  listen_value="$(strip_toml_string_quotes "$(toml_get_raw "$CONFIG_PATH" server listen)")"
  if [ -n "$listen_value" ] && [ "$listen_value" != "${listen_value%:*}" ]; then
    LISTEN_HOST_DEFAULT_VALUE="${listen_value%:*}"
    LISTEN_PORT_DEFAULT_VALUE="${listen_value##*:}"
  fi

  public_base_url="$(strip_toml_string_quotes "$(toml_get_raw "$CONFIG_PATH" server public_base_url)")"
  case "$public_base_url" in
    http://*)
      PUBLIC_SCHEME_DEFAULT_VALUE="http"
      PUBLIC_HOST_DEFAULT_VALUE="${public_base_url#http://}"
      ;;
    https://*)
      PUBLIC_SCHEME_DEFAULT_VALUE="https"
      PUBLIC_HOST_DEFAULT_VALUE="${public_base_url#https://}"
      ;;
  esac

  readonly_username="$(strip_toml_string_quotes "$(toml_get_raw "$CONFIG_PATH" auth username)")"
  if [ -n "$readonly_username" ]; then
    READONLY_USERNAME_DEFAULT_VALUE="$readonly_username"
  fi
  readonly_password="$(strip_toml_string_quotes "$(toml_get_raw "$CONFIG_PATH" auth password)")"
  if [ -n "$readonly_password" ]; then
    READONLY_PASSWORD_DEFAULT_VALUE="$readonly_password"
  fi

  value="$(trim_whitespace "$(toml_get_raw "$CONFIG_PATH" server stale_after_secs)")"
  [ -n "$value" ] && SERVER_STALE_AFTER_SECS="$value"
  value="$(trim_whitespace "$(toml_get_raw "$CONFIG_PATH" server ping_interval_secs)")"
  [ -n "$value" ] && SERVER_PING_INTERVAL_SECS="$value"
  value="$(trim_whitespace "$(toml_get_raw "$CONFIG_PATH" server max_message_bytes)")"
  [ -n "$value" ] && SERVER_MAX_MESSAGE_BYTES="$value"
  value="$(trim_whitespace "$(toml_get_raw "$CONFIG_PATH" ws max_total_connections)")"
  [ -n "$value" ] && WS_MAX_TOTAL_CONNECTIONS="$value"
  value="$(trim_whitespace "$(toml_get_raw "$CONFIG_PATH" ws max_connections_per_ip)")"
  [ -n "$value" ] && WS_MAX_CONNECTIONS_PER_IP="$value"
  value="$(trim_whitespace "$(toml_get_raw "$CONFIG_PATH" ws auth_fail_window_secs)")"
  [ -n "$value" ] && WS_AUTH_FAIL_WINDOW_SECS="$value"
  value="$(trim_whitespace "$(toml_get_raw "$CONFIG_PATH" ws auth_fail_max_attempts)")"
  [ -n "$value" ] && WS_AUTH_FAIL_MAX_ATTEMPTS="$value"
  value="$(trim_whitespace "$(toml_get_raw "$CONFIG_PATH" ws auth_block_secs)")"
  [ -n "$value" ] && WS_AUTH_BLOCK_SECS="$value"
  value="$(trim_whitespace "$(toml_get_raw "$CONFIG_PATH" ui refresh_interval_secs)")"
  [ -n "$value" ] && UI_REFRESH_INTERVAL_SECS="$value"
  value="$(trim_whitespace "$(toml_get_raw "$CONFIG_PATH" filters ignored_filesystems)")"
  [ -n "$value" ] && IGNORED_FILESYSTEMS_RAW="$value"
}

render_server_config() {
  cat <<EOF
[server]
listen = "${LISTEN_HOST}:${LISTEN_PORT}"
public_base_url = "${PUBLIC_SCHEME}://${PUBLIC_HOST}"
node_registry_path = "${CONFIG_DIR}/server.json"
history_db_path = "${DATA_DIR}/history.sqlite3"
snapshot_path = "${DATA_DIR}/snapshot.json"
stale_after_secs = ${SERVER_STALE_AFTER_SECS}
ping_interval_secs = ${SERVER_PING_INTERVAL_SECS}
max_message_bytes = ${SERVER_MAX_MESSAGE_BYTES}

[auth]
username = "${READONLY_USERNAME}"
password = "${READONLY_PASSWORD}"

[ws]
max_total_connections = ${WS_MAX_TOTAL_CONNECTIONS}
max_connections_per_ip = ${WS_MAX_CONNECTIONS_PER_IP}
auth_fail_window_secs = ${WS_AUTH_FAIL_WINDOW_SECS}
auth_fail_max_attempts = ${WS_AUTH_FAIL_MAX_ATTEMPTS}
auth_block_secs = ${WS_AUTH_BLOCK_SECS}

[ui]
refresh_interval_secs = ${UI_REFRESH_INTERVAL_SECS}

[filters]
ignored_filesystems = ${IGNORED_FILESYSTEMS_RAW}
EOF
}

[ "$(id -u)" -eq 0 ] || fail "please run as root"

need_cmd awk
need_cmd chmod
need_cmd chown
need_cmd curl
need_cmd id
need_cmd install
need_cmd mkdir
need_cmd mktemp
need_cmd mv
need_cmd od
need_cmd rm
need_cmd sed
need_cmd systemctl
need_cmd tr
need_cmd uname

clear_screen

printf '%s\n' "XiMonitor Server Installer" >/dev/tty
printf '%s\n' "This script installs the latest XiMonitor server release from GitHub." >/dev/tty
printf '\n' >/dev/tty

existing_install_root="$(detect_existing_install_root)"
if [ -n "$existing_install_root" ]; then
  INSTALL_ROOT_DEFAULT="$existing_install_root"
fi

INSTALL_ROOT="$(prompt_required "Install root directory" "$INSTALL_ROOT_DEFAULT")"

CONFIG_DIR="$INSTALL_ROOT/config"
DATA_DIR="$INSTALL_ROOT/data"
CONFIG_PATH="$CONFIG_DIR/server.toml"
REGISTRY_PATH="$CONFIG_DIR/server.json"

existing_install=0
if [ -e "$CONFIG_PATH" ] || [ -e "$UNIT_PATH" ] || [ -e "$BIN_PATH" ]; then
  existing_install=1
fi

if [ "$MODE" = "auto" ]; then
  if [ "$existing_install" -eq 1 ]; then
    MODE="upgrade"
  else
    MODE="install"
  fi
fi

MODE="$(prompt_mode "$MODE")"

if [ "$MODE" = "upgrade" ] && [ "$existing_install" -ne 1 ]; then
  fail "upgrade mode requires an existing XiMonitor server installation"
fi

if [ "$MODE" = "install" ] && [ "$existing_install" -eq 1 ]; then
  if ! confirm_default_no "Existing XiMonitor files detected. Overwrite them?"; then
    fail "aborted by user"
  fi
fi

SERVER_STALE_AFTER_SECS="20"
SERVER_PING_INTERVAL_SECS="10"
SERVER_MAX_MESSAGE_BYTES="65536"
WS_MAX_TOTAL_CONNECTIONS="1024"
WS_MAX_CONNECTIONS_PER_IP="32"
WS_AUTH_FAIL_WINDOW_SECS="300"
WS_AUTH_FAIL_MAX_ATTEMPTS="12"
WS_AUTH_BLOCK_SECS="900"
UI_REFRESH_INTERVAL_SECS="5"
IGNORED_FILESYSTEMS_RAW='["tmpfs", "devtmpfs", "overlay"]'

LISTEN_HOST_DEFAULT_VALUE="$LISTEN_HOST_DEFAULT"
LISTEN_PORT_DEFAULT_VALUE="$(random_port)"
PUBLIC_HOST_DEFAULT_VALUE=""
PUBLIC_SCHEME_DEFAULT_VALUE="https"
READONLY_USERNAME_DEFAULT_VALUE="viewer"
READONLY_PASSWORD_DEFAULT_VALUE="$(random_hex 16)"

if [ "$MODE" = "upgrade" ]; then
  load_existing_server_defaults
fi

LISTEN_HOST="$(prompt_required "Listen host" "$LISTEN_HOST_DEFAULT_VALUE")"
LISTEN_PORT="$(prompt_required "Listen port" "$LISTEN_PORT_DEFAULT_VALUE")"
PUBLIC_HOST="$(prompt_required "Public domain or IP" "$PUBLIC_HOST_DEFAULT_VALUE")"
PUBLIC_SCHEME="$(prompt_required "Public scheme" "$PUBLIC_SCHEME_DEFAULT_VALUE")"
READONLY_USERNAME="$(prompt_required "Readonly username" "$READONLY_USERNAME_DEFAULT_VALUE")"
READONLY_PASSWORD="$(prompt_required "Readonly password" "$READONLY_PASSWORD_DEFAULT_VALUE")"

validate_port "$LISTEN_PORT"
validate_scheme "$PUBLIC_SCHEME"
validate_no_whitespace "install root directory" "$INSTALL_ROOT"
validate_no_whitespace "public host" "$PUBLIC_HOST"

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64)
    TARGET="x86_64-unknown-linux-musl"
    ;;
  aarch64|arm64)
    TARGET="aarch64-unknown-linux-musl"
    ;;
  *)
    fail "unsupported architecture: $ARCH"
    ;;
esac

ARTIFACT_NAME="ximonitor-server-$TARGET"
resolve_release_base_url
DOWNLOAD_URL="$BASE_URL/$ARTIFACT_NAME"

mkdir -p "$INSTALL_ROOT" "$CONFIG_DIR" "$DATA_DIR"
chown root:root "$INSTALL_ROOT" "$CONFIG_DIR" "$DATA_DIR"
chmod 0755 "$INSTALL_ROOT"
chmod 0700 "$CONFIG_DIR" "$DATA_DIR"

TMP_BIN="$(mktemp "$INSTALL_ROOT/ximonitor-server.XXXXXX")"
TMP_SHA256="$(mktemp "$INSTALL_ROOT/ximonitor-sha256.XXXXXX")"

EXPECTED_SHA256="$(fetch_expected_sha256 "$ARTIFACT_NAME")"

printf '%s\n' "Downloading $DOWNLOAD_URL"
curl -fsSL "$DOWNLOAD_URL" -o "$TMP_BIN" || fail "failed to download server binary"
ACTUAL_SHA256="$(calculate_sha256 "$TMP_BIN")"
[ "$ACTUAL_SHA256" = "$EXPECTED_SHA256" ] || fail "downloaded server checksum mismatch"

install -o root -g root -m 0755 "$TMP_BIN" "$BIN_PATH"
render_server_config >"$CONFIG_PATH"
chmod 0600 "$CONFIG_PATH"

if [ ! -f "$REGISTRY_PATH" ]; then
  printf '%s\n' '{"nodes":[],"install_sessions":[]}' >"$REGISTRY_PATH"
fi
chmod 0600 "$REGISTRY_PATH"

cat >"$UNIT_PATH" <<EOF
[Unit]
Description=XiMonitor Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=$BIN_PATH --config $CONFIG_PATH
Restart=always
RestartSec=3
WorkingDirectory=$INSTALL_ROOT
User=root
Group=root
NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true
ProtectSystem=full
ReadWritePaths=$INSTALL_ROOT

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable "$SERVICE_NAME.service"
systemctl restart "$SERVICE_NAME.service"

clear_screen
if [ "$MODE" = "upgrade" ]; then
  printf '%s\n' "XiMonitor server upgraded and restarted." >/dev/tty
else
  printf '%s\n' "XiMonitor server installed and started." >/dev/tty
fi
printf '%s\n' "Binary: $BIN_PATH" >/dev/tty
printf '%s\n' "Config: $CONFIG_PATH" >/dev/tty
printf '%s\n' "Registry: $REGISTRY_PATH" >/dev/tty
printf '%s\n' "Readonly username: $READONLY_USERNAME" >/dev/tty
printf '%s\n' "Readonly password: $READONLY_PASSWORD" >/dev/tty
printf '%s\n' "Public base URL: ${PUBLIC_SCHEME}://${PUBLIC_HOST}" >/dev/tty
printf '\n' >/dev/tty
printf '%s\n' "Next step: enroll an agent from this server with:" >/dev/tty
printf '%s\n' "  $BIN_PATH --config $CONFIG_PATH install-agent --node-id hk-01 --node-label \"Hong Kong 01\"" >/dev/tty
