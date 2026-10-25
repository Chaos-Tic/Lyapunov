#!/usr/bin/env bash
# Regenerates docs/media from canned demo data (tests/fixtures/demo): no real
# card, window or path is ever shown.
#
# Needs: a built desktop (cmake --build build), ffmpeg, ImageMagick (magick) and
# Inkscape (for the banners).
#
# Art direction (Chaos-Tic Liquid Glass): the same accents everywhere, only the
# surface flips. Lyapunov is a desktop tool, so light is the default; every hero
# visual still ships light AND dark. Colours come from Chaostic-Skins
# skins/liquid-glass/tokens.css:
#   ink   #151820 (light) / #f3f4f8 (dark)   muted #4a5060 / #bcc2d0
#   blue  #40baf0   amber #ffa840   violet #ce6cf0
# Banner text is set in Geist and flattened to vector paths, so GitHub shows it
# the same without shipping or fetching the font at view time.
set -euo pipefail
cd "$(dirname "$0")/.."
out=docs/media
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

have() { command -v "$1" >/dev/null 2>&1; }
for tool in ffmpeg magick inkscape; do
    have "$tool" || { echo "render-docs: missing required tool: $tool" >&2; exit 1; }
done

# The desktop must be built first; this script renders, it does not compile.
if [[ ! -x build/lyapunov-desktop ]]; then
    echo "render-docs: build the desktop first:" >&2
    echo "  cmake -S . -B build -DCMAKE_BUILD_TYPE=Release && cmake --build build" >&2
    exit 1
fi

export QT_QPA_PLATFORM=offscreen LYAPUNOV_DEMO_DIR=$PWD/tests/fixtures/demo LYAPUNOV_LANGUAGE=en

# --- 1. Screenshots, both themes -------------------------------------------
# --render drops stack/capture/plan/update/report/empty.png for the chosen
# theme. Light keeps the plain names (the desktop tool's default); dark gets the
# -dark suffix. French still is light only.
for look in light dark; do
    LYAPUNOV_THEME=$look LYAPUNOV_REDUCED_MOTION=1 build/lyapunov-desktop --render "$work/$look"
done
LYAPUNOV_LANGUAGE=fr LYAPUNOV_THEME=light LYAPUNOV_REDUCED_MOTION=1 build/lyapunov-desktop --render "$work/fr"

cp "$work/light/stack.png"   "$out/stack-light.png"
cp "$work/light/capture.png" "$out/capture.png"
cp "$work/light/plan.png"    "$out/plan.png"
cp "$work/light/update.png"  "$out/update.png"
cp "$work/light/report.png"  "$out/report.png"
cp "$work/dark/stack.png"    "$out/stack-dark.png"
cp "$work/dark/capture.png"  "$out/capture-dark.png"
cp "$work/dark/plan.png"     "$out/plan-dark.png"
cp "$work/dark/update.png"   "$out/update-dark.png"
cp "$work/dark/report.png"   "$out/report-dark.png"
cp "$work/fr/stack.png"      "$out/stack-fr.png"

# --- 2. Hero GIF, both themes ----------------------------------------------
# The whole function in one loop: each card opens in place to show its apps,
# folders and the Resume button, while the others stay as bands. --render-frames
# honours LYAPUNOV_THEME, so the dark hero is a real offscreen render, not a
# recolour. 600 px wide, ~20 fps, kept well under 3 MB.
build_hero_gif() {
    local look=$1 dest=$2
    LYAPUNOV_THEME=$look build/lyapunov-desktop --render-frames "$work/frames-$look"
    ffmpeg -loglevel error -y -framerate 30 -i "$work/frames-$look/%04d.png" \
        -vf "fps=20,crop=880:700:0:0,scale=600:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
        -loop 0 "$dest"
}
build_hero_gif light "$out/stack.gif"
build_hero_gif dark  "$out/stack-dark.gif"

# --- 3. Banners: vector header, both themes --------------------------------
# The authored sources (banner-src.svg, banner-dark-src.svg) set their text in
# Geist; Inkscape flattens that text to paths so GitHub needs no font at view
# time. Geist (OFL-1.1) is vendored in docs/fonts and used offline here: nothing
# is installed on the host and nothing is fetched. Point GEIST_DIR elsewhere to
# override.
geist_src=${GEIST_DIR:-$PWD/docs/fonts}
fontdir="$work/share/fonts"
mkdir -p "$fontdir"
cp "$geist_src"/Geist-Regular.ttf "$geist_src"/Geist-SemiBold.ttf \
   "$geist_src"/GeistMono-Regular.ttf "$fontdir"/
for look in light dark; do
    src="$out/banner-src.svg"; dest="$out/banner.svg"
    [[ $look == dark ]] && { src="$out/banner-dark-src.svg"; dest="$out/banner-dark.svg"; }
    XDG_DATA_HOME="$work/share" HOME="$work" inkscape "$src" \
        --export-type=svg --export-plain-svg --export-text-to-path -o "$dest" >/dev/null 2>&1
    if grep -q '<text' "$dest"; then
        echo "render-docs: banner text not flattened in $dest (Geist not found?)" >&2
        exit 1
    fi
done

# --- 4. Trim the PNGs ------------------------------------------------------
for f in "$out"/*.png; do magick "$f" -strip -define png:compression-level=9 "$f"; done

echo "render-docs: done"
ls -l "$out"
