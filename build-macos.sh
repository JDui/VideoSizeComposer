#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

for tool in ffmpeg ffprobe; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool is required. Refusing to create a macOS app that cannot inspect or encode video." >&2
    exit 1
  fi
done

pnpm tauri build --bundles app

APP="$ROOT/src-tauri/target/release/bundle/macos/VideoSize Composer.app"
MACOS_DIR="$APP/Contents/MacOS"
if [[ ! -d "$MACOS_DIR" ]]; then
  echo "macOS bundle was not created at: $APP" >&2
  exit 1
fi

cp "$(command -v ffmpeg)" "$MACOS_DIR/ffmpeg"
cp "$(command -v ffprobe)" "$MACOS_DIR/ffprobe"
chmod 755 "$MACOS_DIR/ffmpeg" "$MACOS_DIR/ffprobe"
codesign --force --deep --sign - "$APP"

echo "macOS app with FFmpeg tools:"
echo "$APP"
