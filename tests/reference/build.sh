#!/usr/bin/env bash
# Builds the unmodified upstream PDQ reference at the pinned commit with the harness and prints
# the reference values for the synthetic frames. Output is committed as tests/fixtures/reference.txt.
#
# Requires: gh (authenticated, read access to public repos), docker, cargo.
# Docker use: one container, uniquely named, --rm; it touches nothing else.
set -euo pipefail
SHA=07b82cb6e87b7e0ac7fc2a01d865df5db10ee1f2
REPO=repo
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# file path under pdq/cpp/ -> upstream blob SHA (from docs/provenance/pdq-vetting.md)
declare -A BLOB=(
  [hashing/pdqhashing.cpp]=82c5e7132c51ae6c7bb73ac645b3903f88b155e1
  [hashing/pdqhashing.h]=edb64172f6038ed8b166288aaca450d16ee3e70a
  [hashing/torben.cpp]=ae1d059596b9b6f367f647fe1899ee8bd9ae1142
  [hashing/torben.h]=acb06097f2aa48259b8f7fc86043ee034c1c59a3
  [downscaling/downscaling.cpp]=03a62e3bbc8a1b4d53bfa0a9616e4a117c11a7ab
  [downscaling/downscaling.h]=e3eb14b3322171c6080b753a4daab6858ec34715
  [common/pdqbasetypes.h]=c6d3a3e685cc2333503ee3ebeeee159bc6b6bc91
  [common/pdqhashtypes.cpp]=7e6114c709f78f14829645edee429e8495483ed4
  [common/pdqhashtypes.h]=02051b96eb2d5f38be282a1d97c5acb1a9d48cae
  [common/pdqhamming.h]=9d91670dce0a369e196b8bf67e6fa4a6ea10e8ff
  [common/pdqhamming.cpp]=a59911fdb396976edee70b669e558e246a208a7e
)
for f in "${!BLOB[@]}"; do
  mkdir -p "$work/src/pdq/cpp/$(dirname "$f")"
  gh api "repos/facebook/ThreatExchange/contents/pdq/cpp/$f?ref=$SHA" --jq .content | base64 -d > "$work/src/pdq/cpp/$f"
  got="$(git hash-object "$work/src/pdq/cpp/$f")"
  [ "$got" = "${BLOB[$f]}" ] || { echo "blob mismatch for $f: $got" >&2; exit 1; }
done

(cd "$root" && SAFEIMAGE_FRAMES_DIR="$work/frames" cargo test --test write_frames -- --ignored >/dev/null)
cp "$here/harness.cpp" "$work/src/harness.cpp"

GXX_ARGS='-std=c++17 -O2 -ffp-contract=off -fno-fast-math -I src src/harness.cpp src/pdq/cpp/hashing/pdqhashing.cpp src/pdq/cpp/hashing/torben.cpp src/pdq/cpp/downscaling/downscaling.cpp src/pdq/cpp/common/pdqhashtypes.cpp src/pdq/cpp/common/pdqhamming.cpp'

if [ "${SAFEIMAGE_REF_LOCAL_GXX:-}" = 1 ]; then
  # CI path: the runner's own g++ (no container). The result must equal the committed fixture.
  (cd "$work" && g++ $GXX_ARGS -o harness && ./harness frames $(cat frames/args.txt))     > "$root/tests/fixtures/reference.txt"
else
  name="safeimage-ref-$$"
  MSYS_NO_PATHCONV=1 docker run --rm --name "$name" -v "$(cygpath -m "$work" 2>/dev/null || echo "$work"):/w" -w /w rust:latest     bash -c "g++ $GXX_ARGS -o harness && ./harness frames \$(cat frames/args.txt)"     > "$root/tests/fixtures/reference.txt"
fi
echo "wrote tests/fixtures/reference.txt (upstream $SHA)"
