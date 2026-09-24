#!/usr/bin/env bash
# Build the source with solana-verify and compare the mainnet bytecode hash.
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
export LC_ALL=C

python3 ci/check-build.py --source-only
if [[ -e build-output ]]; then
  echo 'build-output already exists. Use a fresh checkout.' >&2
  exit 1
fi
mkdir build-output
work_dir=$(mktemp -d)
trap 'rm -rf -- "$work_dir"' EXIT
verifier="$work_dir/solana-verify"
rpc_url=https://api.mainnet-beta.solana.com
program_id=54bCSEN5dMFbYyT8v5WSP1JZzzKkBGY1yLwMhiWfC9WV

curl --fail --location --retry 4 --retry-all-errors --connect-timeout 20 --max-time 180 \
  --output "$verifier" \
  https://github.com/solana-foundation/solana-verifiable-build/releases/download/v0.5.2/solana-verify-0.5.2-linux
printf '%s  %s\n' b9d5ccce9634dc14269cab3e7a58649bc141b3306e7735a94478c936341f211f "$verifier" | sha256sum -c -
chmod 755 "$verifier"
"$verifier" --version | tee build-output/verifier-version.txt

docker build --pull --no-cache --platform linux/amd64 \
  --tag stonkwheel-treasury-builder:verify . 2>&1 | tee build-output/image-build.log
image_id=$(docker image inspect --format '{{.Id}}' stonkwheel-treasury-builder:verify)
printf '%s\n' "$image_id" > build-output/builder-image-id.txt

mkdir -p "$work_dir/source/src"
cp Cargo.toml Cargo.lock build.sh programme.json "$work_dir/source/"
cp src/lib.rs "$work_dir/source/src/"
"$verifier" --url "$rpc_url" build "$work_dir/source" \
  --library-name stonk_vault --base-image "$image_id" \
  2>&1 | tee build-output/verifier-build.log

cp "$work_dir/source/target/deploy/stonk_vault.so" build-output/stonk_vault.so
"$verifier" --url "$rpc_url" get-executable-hash build-output/stonk_vault.so \
  | tee build-output/executable-verifier-hash.txt
"$verifier" --url "$rpc_url" get-program-hash "$program_id" \
  | tee build-output/onchain-verifier-hash.txt

python3 - <<'PYREPORT'
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

root = Path.cwd()
out = root / 'build-output'
identity = json.loads((root / 'programme.json').read_text())
binary = (out / 'stonk_vault.so').read_bytes()
binary_sha = hashlib.sha256(binary).hexdigest()
trimmed_sha = hashlib.sha256(binary.rstrip(b'\0')).hexdigest()
executable_hash = (out / 'executable-verifier-hash.txt').read_text().strip()
onchain_hash = (out / 'onchain-verifier-hash.txt').read_text().strip()
version = (out / 'verifier-version.txt').read_text().strip()
image_id = (out / 'builder-image-id.txt').read_text().strip()
commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
if (len(binary), binary_sha, trimmed_sha) != (
        identity['binaryBytes'], identity['binarySha256'], identity['solanaVerifyHash']):
    raise SystemExit('Verifier build differs from the reference ELF.')
if not executable_hash == onchain_hash == trimmed_sha:
    raise SystemExit('The executable, CLI and mainnet hashes differ.')
if version != 'solana-verify 0.5.2' or not re.fullmatch(r'sha256:[0-9a-f]{64}', image_id):
    raise SystemExit('Unexpected verifier version or builder image ID.')
if os.environ.get('GITHUB_SHA', commit) != commit:
    raise SystemExit('The checkout differs from the selected workflow commit.')
for name, expected in identity['sourceFiles'].items():
    if hashlib.sha256((root / name).read_bytes()).hexdigest() != expected:
        raise SystemExit('Source changed during verification: ' + name)
report = {
    'schemaVersion': 1,
    'result': 'verifier_build_matches_mainnet_hash',
    'checkedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'sourceCommit': commit,
    'program': identity['program'],
    'sourceFiles': identity['sourceFiles'],
    'verifierVersion': version,
    'verifierBinarySha256': 'b9d5ccce9634dc14269cab3e7a58649bc141b3306e7735a94478c936341f211f',
    'verifierSourceCommit': 'f8cfe2f834f4334aad9c60a29284de0ba396f829',
    'builderImageId': image_id,
    'platformToolsVersion': identity['platformToolsVersion'],
    'platformToolsArchiveSha256': identity['platformToolsArchiveSha256'],
    'binaryBytes': len(binary),
    'binarySha256': binary_sha,
    'executableVerifierHash': executable_hash,
    'onchainVerifierHash': onchain_hash,
    'rpc': 'https://api.mainnet-beta.solana.com',
}
(out / 'verification.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
summary = os.environ.get('GITHUB_STEP_SUMMARY')
if summary:
    with open(summary, 'a') as stream:
        stream.write('## Treasury bytecode comparison\n\n')
        stream.write('A source build using solana-verify matches the mainnet bytecode hash.\n\n')
        stream.write(f'- Source commit: `{commit}`\n- CLI: `{version}`\n')
        stream.write(f'- ELF bytes: {len(binary)}\n- ELF SHA-256: `{binary_sha}`\n')
        stream.write(f'- Mainnet verifier hash: `{onchain_hash}`\n')
PYREPORT
