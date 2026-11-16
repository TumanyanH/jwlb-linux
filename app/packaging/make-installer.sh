#!/bin/sh
# Builds dist/StudyLibrary-Installer.run: one file that installs on any Linux distribution.
# The program is built against glibc 2.17 (2012) with cargo-zigbuild, so it runs on old and
# new distributions alike; windowing (X11/Wayland) and OpenGL are loaded at run time.
set -e
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
STAGE=target/installer-stage
rm -rf "$STAGE" && mkdir -p "$STAGE/bin/x86_64" "$STAGE/bin/aarch64" dist

cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17
cp target/x86_64-unknown-linux-gnu/release/study-library "$STAGE/bin/x86_64/"
if cargo zigbuild --release --target aarch64-unknown-linux-gnu.2.17; then
  cp target/aarch64-unknown-linux-gnu/release/study-library "$STAGE/bin/aarch64/"
else
  echo "warning: arm64 build failed; installer will be x86_64 only"
fi
target/x86_64-unknown-linux-gnu/release/study-library --write-icon "$STAGE/study-library.png"

OUT=dist/StudyLibrary-Installer.run
cat packaging/installer-header.sh > "$OUT"
tar -czf - -C "$STAGE" . >> "$OUT"
chmod 755 "$OUT"
cp target/x86_64-unknown-linux-gnu/release/study-library dist/study-library-x86_64
ls -la dist
