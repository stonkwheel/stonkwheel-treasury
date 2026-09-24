#!/usr/bin/env bash
# Reproduce the reference ELF twice in fresh containers.
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
export LC_ALL=C

python3 ci/check-build.py --source-only
if [[ -e build-output ]]; then
  echo 'build-output already exists. Use a fresh checkout for a new build.' >&2
  exit 1
fi
mkdir -p build-output/run-1 build-output/run-2

input_dir=$(mktemp -d)
trap 'rm -rf -- "$input_dir"' EXIT
mkdir -p "$input_dir/src"
cp Cargo.toml Cargo.lock build.sh programme.json "$input_dir/"
cp src/lib.rs "$input_dir/src/"

# The Docker context allowlist contains only the Dockerfile and compiler adapter.
docker build --pull --no-cache --platform linux/amd64 \
  --tag stonkwheel-treasury-builder:local . 2>&1 | tee build-output/image-build.log
image_id=$(docker image inspect --format '{{.Id}}' stonkwheel-treasury-builder:local)
printf '%s\n' "$image_id" > build-output/builder-image-id.txt

for attempt in 1 2; do
  # Each fresh container starts with empty Cargo and compilation caches.
  docker run --rm --platform linux/amd64 \
    --mount "type=bind,source=$input_dir,target=/input,readonly" \
    --mount "type=bind,source=$PWD/build-output/run-$attempt,target=/result" \
    "$image_id" bash -euo pipefail -c '
      mkdir -p /build/src
      cp /input/Cargo.toml /input/Cargo.lock /input/build.sh /input/programme.json /build/
      cp /input/src/lib.rs /build/src/
      cd /build
      cargo build-sbf -- --locked
      cmp artifacts/stonk_vault.so target/deploy/stonk_vault.so
      cp artifacts/stonk_vault.so /result/stonk_vault.so
      chmod 644 /result/stonk_vault.so
    ' 2>&1 | tee "build-output/run-$attempt/build.log"
done

python3 ci/check-build.py --image-id "$image_id" \
  --binary build-output/run-1/stonk_vault.so \
  --binary build-output/run-2/stonk_vault.so \
  --report build-output/summary.json
