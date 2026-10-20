# Media

Every image here is generated, never captured from a real desktop: the window
renders itself offscreen from the demo data in `tests/fixtures/demo`, so no
real card, window title or path appears.

| File | Made by |
| --- | --- |
| `banner.svg`, `banner-dark.svg` | hand-written SVG (the README header) |
| `stack-light.png`, `stack-dark.png`, `stack-fr.png`, `capture.png`, `plan.png`, `report.png` | `scripts/render-docs.sh` (`lyapunov-desktop --render`) |
| `stack.gif` | `scripts/render-docs.sh` (`lyapunov-desktop --render-frames`, then ffmpeg) |

Regenerate after a visual change: `cmake --build build && scripts/render-docs.sh`.
