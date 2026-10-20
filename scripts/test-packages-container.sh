#!/usr/bin/env bash
# Builds the .deb, .rpm and Arch packages, each in its own distribution, then
# installs, checks and removes each one in clean containers, as CI does:
#   scripts/test-packages-container.sh            # every format
#   scripts/test-packages-container.sh deb rpm    # some of them
# Needs Docker or Podman. Packages land in dist/.
set -euo pipefail
cd "$(dirname "$0")/.."
engine=$(command -v docker || command -v podman) || { echo "Docker or Podman is required" >&2; exit 2; }
formats=("$@")
[[ ${#formats[@]} -gt 0 ]] || formats=(deb rpm arch)
declare -A builder=([deb]=debian:12 [rpm]=fedora:latest [arch]=archlinux:latest)
declare -A testers=([deb]="debian:12 debian:stable ubuntu:24.04" [rpm]="fedora:latest" [arch]="archlinux:latest")
declare -A glob=([deb]='dist/lyapunov_*.deb' [rpm]='dist/lyapunov-*.rpm' [arch]='dist/lyapunov-*.pkg.tar.zst')
failed=()
for f in "${formats[@]}"; do
    echo "== build $f in ${builder[$f]}"
    rm -f ${glob[$f]}
    # A copy of the tree, so the container never writes into the checkout.
    if ! "$engine" run --rm -v "$PWD:/src:ro" -v "$PWD/dist:/out" "${builder[$f]}" bash -c \
        "cp -r /src /tmp/w && cd /tmp/w && rm -rf target build dist && bash packaging/linux/build-package.sh $f >/tmp/build.log 2>&1 || { tail -40 /tmp/build.log; exit 1; }; cp dist/lyapunov* /out/"; then
        failed+=("build-$f"); continue
    fi
    for image in ${testers[$f]}; do
        echo "== test $f in $image"
        pkg=$(ls ${glob[$f]} | head -1)
        "$engine" run --rm -v "$PWD:/src:ro" "$image" bash -c "bash /src/packaging/linux/test-package.sh /src/$pkg >/tmp/t.log 2>&1 && tail -1 /tmp/t.log || { tail -30 /tmp/t.log; exit 1; }" || failed+=("$f@$image")
    done
done
if [[ ${#failed[@]} -gt 0 ]]; then echo "FAILED: ${failed[*]}" >&2; exit 1; fi
echo "All packages passed: ${formats[*]}"
