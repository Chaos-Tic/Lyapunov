# Media

Every image here is generated, never captured from a real desktop: the window
renders itself offscreen from the demo data in `tests/fixtures/demo`, so no
real card, window title or path appears.

Liquid Glass art direction: the same accents everywhere, only the surface flips.
Lyapunov is a desktop tool, so light is the default, but every hero visual ships
light **and** dark (the `-dark` files). The README picks the dark variant through
`prefers-color-scheme`.

| File | Made by |
| --- | --- |
| `banner.svg`, `banner-dark.svg` | `scripts/render-docs.sh` flattens `banner-src.svg` / `banner-dark-src.svg` (authored SVG, Geist text) to vector paths with Inkscape, so GitHub needs no font |
| `banner-src.svg`, `banner-dark-src.svg` | hand-written SVG sources (edit these; the paths above are generated) |
| `stack-light.png`, `stack-dark.png`, `stack-fr.png` | `scripts/render-docs.sh` (`lyapunov-desktop --render`) |
| `capture.png`, `plan.png`, `report.png`, `update.png` and their `-dark` variants | `scripts/render-docs.sh` (`lyapunov-desktop --render`) |
| `stack.gif`, `stack-dark.gif` | `scripts/render-docs.sh` (`lyapunov-desktop --render-frames`, then ffmpeg) — each theme is a real offscreen render |

Geist (OFL-1.1) is vendored in [`../fonts`](../fonts) and used offline when
flattening the banners; nothing is installed on the host or fetched at render
time. `render-docs.sh` needs ffmpeg, ImageMagick and Inkscape.

Regenerate after a visual change: `cmake --build build && scripts/render-docs.sh`.
