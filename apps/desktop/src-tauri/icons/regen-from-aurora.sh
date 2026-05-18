#!/usr/bin/env bash
# Regenerate all desktop app icon variants from source-aurora.svg.
#
# Output:
#   icon.png                — 1024×1024 master PNG (Tauri uses this for Linux)
#   icon.icns               — macOS app bundle icon (built via iconutil)
#   icon.ico                — Windows tray + installer icon (built via Pillow)
#   Square{30,44,71,89,107,142,150,284,310}x{...}Logo.png — Windows Store tiles
#   StoreLogo.png           — Windows Store small logo (50×50)
#   128x128.png, 128x128@2x.png, 32x32.png — additional Tauri variants
#
# Requirements:
#   - Python with cairosvg + Pillow (libcairo bundled, librsvg-free):
#       ~/.claude-python/bin/pip install cairosvg Pillow
#     We use cairosvg over `magick` because the source SVG uses stroked
#     circles with no fill, which ImageMagick's librsvg backend renders
#     incorrectly (rings drop entirely). Cairo + browsers render it
#     correctly, so cairosvg is the source of truth.
#   - iconutil (macOS, builtin) for .icns assembly.

set -euo pipefail
cd "$(dirname "$0")"

SRC="source-aurora.svg"
PY="${PY:-$HOME/.claude-python/bin/python}"

if [[ ! -f "$SRC" ]]; then
  echo "ERROR: $SRC not found in $(pwd)" >&2
  exit 1
fi

if ! "$PY" -c "import cairosvg, PIL" 2>/dev/null; then
  echo "ERROR: cairosvg + Pillow required."
  echo "       Install: $PY -m pip install cairosvg Pillow" >&2
  exit 1
fi

# render_png <size> <out_path>
#   Renders the source SVG at <size>×<size> to <out_path>. The icon is
#   a bare symbol (no plate) so we render on a transparent background.
render_png() {
  local size="$1"; local out="$2"
  echo "  → $out (${size}×${size})"
  "$PY" -c "
import cairosvg
cairosvg.svg2png(
    url='$SRC',
    output_width=$size,
    output_height=$size,
    write_to='$out',
)
"
}

echo "[1/4] Tauri main icons..."
render_png 1024 icon.png
render_png 32   32x32.png
render_png 128  128x128.png
render_png 256  128x128@2x.png

echo "[2/4] Windows Store tile variants..."
render_png 30  Square30x30Logo.png
render_png 44  Square44x44Logo.png
render_png 71  Square71x71Logo.png
render_png 89  Square89x89Logo.png
render_png 107 Square107x107Logo.png
render_png 142 Square142x142Logo.png
render_png 150 Square150x150Logo.png
render_png 284 Square284x284Logo.png
render_png 310 Square310x310Logo.png
render_png 50  StoreLogo.png

echo "[3/4] Windows .ico (multi-resolution)..."
"$PY" -c "
import cairosvg, io
from PIL import Image
sizes = [16, 32, 48, 256]
images = []
for s in sizes:
    buf = io.BytesIO()
    cairosvg.svg2png(url='$SRC', output_width=s, output_height=s, write_to=buf)
    buf.seek(0)
    images.append(Image.open(buf).convert('RGBA'))
images[0].save('icon.ico', format='ICO', sizes=[(s, s) for s in sizes], append_images=images[1:])
print('  → icon.ico')
"

echo "[4/4] macOS .icns (built via iconutil)..."
TMP_PARENT=$(mktemp -d)
ICONSET="$TMP_PARENT/icon.iconset"
mkdir -p "$ICONSET"
for s in 16 32 64 128 256 512 1024; do
  render_png "$s" "$ICONSET/icon_${s}x${s}.png"
done
# Apple's iconset convention also wants @2x variants for retina.
for s in 16 32 128 256 512; do
  s2=$((s * 2))
  if [[ -f "$ICONSET/icon_${s2}x${s2}.png" ]]; then
    cp "$ICONSET/icon_${s2}x${s2}.png" "$ICONSET/icon_${s}x${s}@2x.png"
  fi
done
iconutil -c icns -o icon.icns "$ICONSET"
rm -rf "$TMP_PARENT"
echo "  → icon.icns"

echo
echo "Done. Variants regenerated from $SRC."
echo "Next: rebuild the desktop app (pnpm tauri build) so the new icon"
echo "ships in the .dmg and .msi installers."
