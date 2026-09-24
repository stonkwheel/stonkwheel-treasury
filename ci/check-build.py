#!/usr/bin/env python3
"""Check source hashes and reproducible-build outputs."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PROGRAM = '54bCSEN5dMFbYyT8v5WSP1JZzzKkBGY1yLwMhiWfC9WV'
SIZE = 163048
ELF_HASH = 'd76cfe71ebbb3a642c2f3223eb325962a23e9c97c7bba1be03e8b6d85ebd6d58'
VERIFY_HASH = '7b9ed1d0d1eeb3aeafd767485bd802747acfa361a3a694f0644d5cca08a426fc'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def check_sources():
    identity = json.loads((ROOT / 'programme.json').read_text())
    if (identity['program'], identity['binaryBytes'], identity['binarySha256'],
            identity['solanaVerifyHash']) != (PROGRAM, SIZE, ELF_HASH, VERIFY_HASH):
        raise ValueError('Unexpected programme identity')
    expected_names = {'src/lib.rs', 'Cargo.toml', 'Cargo.lock', 'build.sh'}
    if set(identity['sourceFiles']) != expected_names:
        raise ValueError('Unexpected source-file list')
    for name, expected in identity['sourceFiles'].items():
        path = ROOT / name
        if path.is_symlink() or not path.is_file() or sha(path.read_bytes()) != expected:
            raise ValueError('Source checksum mismatch: ' + name)
    for line in (ROOT / 'SOURCE_MANIFEST.sha256').read_text().splitlines():
        expected, name = line.split('  ', 1)
        path = ROOT / name
        if (not re.fullmatch(r'[0-9a-f]{64}', expected) or Path(name).is_absolute()
                or '..' in Path(name).parts or path.is_symlink()
                or not path.is_file() or sha(path.read_bytes()) != expected):
            raise ValueError('Repository checksum mismatch: ' + name)
    return identity


def check_binary(path):
    data = path.read_bytes()
    result = {'bytes': len(data), 'sha256': sha(data),
              'solanaVerifyHash': sha(data.rstrip(b'\x00'))}
    if (result['bytes'], result['sha256'], result['solanaVerifyHash']) != (
            SIZE, ELF_HASH, VERIFY_HASH):
        raise ValueError('Binary differs from the reference ELF: ' + str(path))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-only', action='store_true')
    parser.add_argument('--binary', type=Path, action='append', default=[])
    parser.add_argument('--report', type=Path)
    parser.add_argument('--image-id')
    args = parser.parse_args()
    identity = check_sources()
    if args.source_only:
        print('Source and build-recipe checksums match the published manifest.')
        return
    if len(args.binary) != 2 or not args.report:
        parser.error('Supply two --binary arguments and one --report path')
    if not args.image_id or not re.fullmatch(r'sha256:[0-9a-f]{64}', args.image_id):
        parser.error('Supply the local builder image ID using --image-id')
    results = [check_binary(path) for path in args.binary]
    if args.binary[0].read_bytes() != args.binary[1].read_bytes():
        raise ValueError('The two fresh builds differ')
    commit = subprocess.check_output(
        ['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip()
    report = {
        'schemaVersion': 1,
        'result': 'two_builds_match_reference_elf',
        'checkedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'program': PROGRAM,
        'sourceCommit': commit,
        'sourceFiles': identity['sourceFiles'],
        'platformToolsVersion': identity['platformToolsVersion'],
        'platformToolsArchiveSha256': identity['platformToolsArchiveSha256'],
        'builderImageId': args.image_id,
        'builds': results,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as out:
            out.write('## Reproducible build result\n\n')
            out.write('Two independent builds match the reference ELF.\n\n')
            out.write(f'- Commit: `{commit}`\n- ELF bytes: {SIZE}\n')
            out.write(f'- SHA-256: `{ELF_HASH}`\n- Verifier hash: `{VERIFY_HASH}`\n\n')
            out.write('Scope: two source builds compared with the reference ELF.\n')


if __name__ == '__main__':
    main()
