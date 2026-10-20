#!/usr/bin/env bash
# Regenerates docs/media from canned demo data (tests/fixtures/demo): no real
# card, window or path is ever shown. Needs a built desktop (cmake --build
# build), ffmpeg and ImageMagick.
set -euo pipefail
cd "$(dirname "$0")/.."
out=docs/media
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export QT_QPA_PLATFORM=offscreen LYAPUNOV_DEMO_DIR=$PWD/tests/fixtures/demo LYAPUNOV_LANGUAGE=en
for look in light dark; do
    LYAPUNOV_THEME=$look LYAPUNOV_REDUCED_MOTION=1 build/lyapunov-desktop --render "$work/$look"
done
LYAPUNOV_LANGUAGE=fr LYAPUNOV_THEME=light LYAPUNOV_REDUCED_MOTION=1 build/lyapunov-desktop --render "$work/fr"
cp "$work/light/stack.png" "$out/stack-light.png"
cp "$work/dark/stack.png" "$out/stack-dark.png"
cp "$work/light/capture.png" "$out/capture.png"
cp "$work/dark/plan.png" "$out/plan.png"
cp "$work/light/report.png" "$out/report.png"
cp "$work/fr/stack.png" "$out/stack-fr.png"
LYAPUNOV_THEME=light build/lyapunov-desktop --render-frames "$work/frames"
ffmpeg -loglevel error -y -framerate 30 -i "$work/frames/%04d.png" \
    -vf "fps=20,crop=880:700:0:0,scale=600:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
    -loop 0 "$out/stack.gif"
# PNG screenshots, lightly compressed.
for f in "$out"/*.png; do magick "$f" -strip -define png:compression-level=9 "$f"; done
ls -l "$out"
