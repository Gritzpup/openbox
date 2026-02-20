#!/bin/bash
set -e

PROJECT_ROOT="/home/ubuntubox/github/openbox"
STATIC_DIR="$PROJECT_ROOT/local-update-server/static"
STATUS_FILE="$STATIC_DIR/build-status.json"

VERSION=$(grep -m 1 "version =" "$PROJECT_ROOT/src-tauri/Cargo.toml" | cut -d '"' -f 2)
PRIVATE_KEY="dW50cnVzdGVkIGNvbW1lbnQ6IHJzaWduIGVuY3J5cHRlZCBzZWNyZXQga2V5ClJXUlRZMEl5eERLQWgrSHRYNWhDYjZPL2xBZ0J5UDg0WHd2MVVQV2IwQWFidkJ4My9TWUFBQkFBQUFBQUFBQUFBQUlBQUFBQWFUdnNJTlN3V1ZKYWNkajd1VDlQci9lejRYQmxiUEZzVlFQTTY2ZlJyVVhocEIwSkV6aGV5S3VIcENWWi91T0Fqa2JiVUhUMHc2SEplRnpRVUFMZnl1TEpnaFl5Nk01OHgzeDdvZ2hmQTNtSUdwMnhwUHVMYnQyR1BHWUVwdnpIT1FhV054UkVSclU9Cg=="
export TAURI_SIGNING_PRIVATE_KEY="$PRIVATE_KEY"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="solidsnake"

update_status() {
    local status=$1
    local conclusion=$2
    mkdir -p "$STATIC_DIR"
    cat > "$STATUS_FILE" << EOF
{
  "status": "$status",
  "conclusion": "$conclusion",
  "version": "v$VERSION"
}
EOF
}

echo "🚀 Starting Local Build for v$VERSION..."
update_status "in_progress" "null"

# Build Frontend
echo "📦 Building Svelte frontend..."
cd "$PROJECT_ROOT"
npm run build

# Build Windows App
echo "🦀 Building Rust backend for Windows..."
# Note: tauri build handles cargo build internally
cd "$PROJECT_ROOT"
npx tauri build --target x86_64-pc-windows-gnu

BUNDLE_DIR="$PROJECT_ROOT/src-tauri/target/x86_64-pc-windows-gnu/release/bundle/nsis"
EXE_NAME="TurboLaunch_${VERSION}_x64-setup.exe"
SIG_FILE="${BUNDLE_DIR}/${EXE_NAME}.sig"

if [ ! -f "${BUNDLE_DIR}/${EXE_NAME}" ]; then
    echo "❌ Error: Installer not found at ${BUNDLE_DIR}/${EXE_NAME}"
    update_status "completed" "failure"
    exit 1
fi

echo "🔏 Signing installer..."
# Standard tauri signer sign usage
npx tauri signer sign "${BUNDLE_DIR}/${EXE_NAME}" > /tmp/tauri_sign_output 2>&1 || true
# Extract signature from file since stdout might be messy
if [ -f "$SIG_FILE" ]; then
    SIGNATURE=$(cat "${SIG_FILE}")
else
    # Fallback attempt
    SIGNATURE=$(grep "Public signature:" -A 1 /tmp/tauri_sign_output | tail -n 1)
fi

if [ -z "$SIGNATURE" ]; then
    echo "❌ Error: Failed to generate signature"
    update_status "completed" "failure"
    exit 1
fi

# Copy to static dir
echo "🚚 Deploying to local update server..."
cp "${BUNDLE_DIR}/${EXE_NAME}" "$STATIC_DIR/"

# Update latest.json
DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
cat > "$STATIC_DIR/latest.json" << EOF
{
  "version": "$VERSION",
  "notes": "Local build v$VERSION",
  "pub_date": "$DATE",
  "platforms": {
    "windows-x86_64": {
      "signature": "$SIGNATURE",
      "url": "http://192.168.1.51:3001/$EXE_NAME"
    }
  }
}
EOF

# Also update NAS if it's the data root
if [ -d "/home/ubuntubox/freenas/Emulation/Josh Program Files (x86)/OpenBox" ]; then
    echo "📂 Updating NAS..."
    cp "${BUNDLE_DIR}/${EXE_NAME}" "/home/ubuntubox/freenas/TurboLaunch_Installer.exe"
    cp "$STATIC_DIR/latest.json" "/home/ubuntubox/freenas/TurboLaunch_Update.json"
fi

echo "✅ Build Complete!"
update_status "completed" "success"
