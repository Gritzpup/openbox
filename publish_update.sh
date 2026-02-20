#!/bin/bash
set -e

# Configuration
VERSION=$(grep -m 1 "version =" src-tauri/Cargo.toml | cut -d '"' -f 2)
# Private Key for signing
PRIVATE_KEY="dW50cnVzdGVkIGNvbW1lbnQ6IHJzaWduIGVuY3J5cHRlZCBzZWNyZXQga2V5ClJXUlRZMEl5STdUT3MrSy85MEhFa1U1YURGZmNWRFlsQVBHbzRtbTZWZ0Eyd282b2llWUFBQkFBQUFBQUFBQUFBQUlBQUFBQS8yZ0RiN3dzRmlybHlnazNvLzA3bXN0UmZEdGhWUEU2WkZ4MzlRZGR4cXNwTFJUMFVRejI0aDhHRGlKbG5QSmRSMno3RFhNV25idmRPdHEwWXAwSnJjOWRvcG13TXc5c01ZTDBlNGkxT3U2K3JmSzdtMXAxK0tSZnVOVFRzcGlDbUxnbm5UTGU0NXM9Cg=="
export TAURI_SIGNING_PRIVATE_KEY="$PRIVATE_KEY"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="solidsnake"

echo "Building TurboLaunch v$VERSION for Windows..."

# Build the app
cd src-tauri
npx @tauri-apps/cli build --target x86_64-pc-windows-gnu

# Paths
BUNDLE_DIR="target/x86_64-pc-windows-gnu/release/bundle/nsis"
EXE_NAME="TurboLaunch_${VERSION}_x64-setup.exe"
SIG_FILE="${BUNDLE_DIR}/${EXE_NAME}.sig"

echo "Update files generated. Copying to NAS..."

# Copy installer to NAS root
cp "${BUNDLE_DIR}/${EXE_NAME}" /home/ubuntubox/freenas/TurboLaunch_Installer.exe

# Generate latest.json for the updater
# This matches the endpoint in tauri.conf.json
DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
SIGNATURE=$(cat "${SIG_FILE}" || echo "manual_sign_required")

mkdir -p ../updater
cat > ../updater/latest.json << EOF
{
  "version": "$VERSION",
  "notes": "Drag & Drop Wizard and Auto-Updater support.",
  "pub_date": "$DATE",
  "platforms": {
    "windows-x86_64": {
      "signature": "$SIGNATURE",
      "url": "https://github.com/Gritzpup/openbox/releases/download/v$VERSION/${EXE_NAME}"
    }
  }
}
EOF

# Copy update json to NAS too just in case
cp ../updater/latest.json /home/ubuntubox/freenas/TurboLaunch_Update.json

echo "Done! TurboLaunch v$VERSION is now on the NAS and updater metadata prepared."
echo "IMPORTANT: To enable auto-updates, you must push the 'updater/latest.json' file to GitHub."
