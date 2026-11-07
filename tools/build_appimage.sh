#!/bin/sh
# Builds Dragonize-x86_64.AppImage.
# The binary is compiled inside Debian bullseye (glibc 2.31) so the AppImage
# runs on older distributions too; GL/Wayland/X11 libs are loaded from the host
# at runtime, as usual for AppImages.
# usage: tools/build_appimage.sh /path/to/appimagetool
set -eu
APPIMAGETOOL=${1:?path to appimagetool}
cd "$(dirname "$0")/.."
# Same rustc as the development build: other versions round glyph coverage
# differently and the rendered text would no longer match pixel for pixel.
RUSTC=1.99.1
podman run --rm -v "$PWD":/src:Z -w /src -e CARGO_TARGET_DIR=/src/target/bullseye \
    docker.io/library/rust:1.91-slim-bullseye \
    sh -c "rustup toolchain install $RUSTC --profile minimal >/dev/null && cargo +$RUSTC build --release --locked"
APPDIR=target/AppDir
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/licenses/dragonize"
cp target/bullseye/release/dragonize "$APPDIR/usr/bin/"
cp assets/fonts/LICENSE-*.txt "$APPDIR/usr/share/licenses/dragonize/"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
HERE=$(dirname "$(readlink -f "$0")")
exec "$HERE/usr/bin/dragonize" "$@"
RUN
chmod +x "$APPDIR/AppRun"
cat > "$APPDIR/dragonize.desktop" <<'DESK'
[Desktop Entry]
Type=Application
Name=Dragonize
Comment=Unofficial configuration tool for M801-RGB and other Holtek-based gaming mice
Exec=dragonize
Icon=dragonize
Categories=Settings;HardwareSettings;
DESK
cp assets/brand/icon.png "$APPDIR/dragonize.png"
cp assets/brand/icon.png "$APPDIR/.DirIcon"
ARCH=x86_64 "$APPIMAGETOOL" --no-appstream "$APPDIR" Dragonize-x86_64.AppImage.new
# rename, so a running copy (FUSE-mounted) does not block the rebuild
mv -f Dragonize-x86_64.AppImage.new Dragonize-x86_64.AppImage
