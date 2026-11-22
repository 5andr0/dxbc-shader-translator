"""Write the patched crates.io crates that the [patch.crates-io] block of Cargo.toml points at.

For each package in dependencies.json this tool takes the official crates.io archive (checked
against its pinned SHA-256 and complete file map), applies the edits of `<name>-<version>.edits.json`
and checks every resulting file against the pinned postimage hashes. The result is written to
`.patched/<name>-<version>-<digest>`, where <digest> is the first 12 hex digits of the SHA-256 of the
postimage file list. Existing directories are kept (pass --verify to check them again).

    python patches/patched_crates.py --download     # first run: fetch the archives from static.crates.io
    python patches/patched_crates.py                # archives already in the Cargo registry cache
    python patches/patched_crates.py --verify       # replay and compare existing directories

Archives are looked up in --cache, the Cargo registry caches ($CARGO_HOME or ~/.cargo) and
~/.cache/dxbc-shader-translator-archives; --download stores fetched archives in the latter.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import sys
import tarfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
MANIFEST = HERE / 'dependencies.json'
SCHEMA = 'dxbc-shader-translator-patched-crates-v1'
DOWNLOADS = Path.home() / '.cache' / 'dxbc-shader-translator-archives'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def safe(name):
    p = PurePosixPath(name)
    if (not name or p.is_absolute() or '\\' in name or ':' in name
            or any(x in ('', '..', '.') for x in name.split('/'))):
        raise SystemExit(f'unsafe package path: {name!r}')
    return p


def crate_dir(package):
    canonical = json.dumps(package['postimage_files'], sort_keys=True, separators=(',', ':'))
    return f"{package['name']}-{package['version']}-{sha(canonical.encode('utf-8'))[:12]}"


def archive_dirs(explicit):
    if explicit:
        return [explicit]
    homes = [Path(os.environ['CARGO_HOME'])] if os.environ.get('CARGO_HOME') else []
    homes.append(Path.home() / '.cargo')
    dirs = []
    for home in homes:
        cache = home / 'registry' / 'cache'
        if cache.is_dir():
            dirs += sorted(p for p in cache.iterdir() if p.is_dir())
    dirs.append(DOWNLOADS)
    return dirs


def find_archive(package, dirs, download):
    for d in dirs:
        path = d / package['archive_file']
        if path.is_file() and sha(path.read_bytes()) == package['archive_sha256']:
            return path
    if not download:
        raise SystemExit(f"missing archive {package['archive_file']} (searched {', '.join(map(str, dirs))}); "
                         'pass --cache <dir> or --download')
    import urllib.request
    url = package['official_archive_url']
    if url != f"https://static.crates.io/crates/{package['name']}/{package['archive_file']}":
        raise SystemExit('non-official archive URL')
    with urllib.request.urlopen(url, timeout=60) as response:
        raw = response.read(64 * 1024 * 1024 + 1)
    if sha(raw) != package['archive_sha256']:
        raise SystemExit(f'download checksum mismatch: {url}')
    DOWNLOADS.mkdir(parents=True, exist_ok=True)
    path = DOWNLOADS / package['archive_file']
    path.write_bytes(raw)
    return path


def archive_files(path, package):
    raw = path.read_bytes()
    if sha(raw) != package['archive_sha256']:
        raise SystemExit('archive checksum mismatch')
    files, seen, prefix = {}, set(), package['archive_root'] + '/'
    with tarfile.open(fileobj=io.BytesIO(raw), mode='r:gz') as archive:
        for member in archive:
            if member.isdir():
                continue
            if not member.isfile() or not member.name.startswith(prefix):
                raise SystemExit('non-file or wrong archive root')
            name = member.name[len(prefix):]
            safe(name)
            if name.casefold() in seen:
                raise SystemExit('duplicate package path')
            seen.add(name.casefold())
            files[name] = archive.extractfile(member).read()
    if {n: sha(b) for n, b in files.items()} != package['base_files']:
        raise SystemExit(f"archive file map mismatch: {package['name']}")
    return files


def apply_edit(base, edit):
    if sha(base) != edit['before_sha256']:
        raise SystemExit('edit preimage changed')
    base = base.replace(b'\r\n', b'\n')
    if sha(base) != edit['before_lf_sha256']:
        raise SystemExit('normalized edit preimage changed')
    parts, cursor = [], 0
    for span in edit['spans']:
        at = span['byte_offset']
        end = at + span['delete_bytes']
        if not cursor <= at <= end <= len(base) or sha(base[at:end]) != span['deleted_sha256']:
            raise SystemExit('edit span preimage changed')
        parts.extend([base[cursor:at], span['insert_utf8'].encode('utf-8')])
        cursor = end
    out = b''.join(parts) + base[cursor:]
    if sha(out) != edit['after_lf_sha256']:
        raise SystemExit('normalized edit postimage changed')
    policy = edit['newline_policy']
    if policy == 'crlf':
        out = out.replace(b'\n', b'\r\n')
    elif policy != 'lf':
        runs = policy['crlf_runs']
        lines = out.split(b'\n')
        pieces = []
        for i, line in enumerate(lines):
            pieces.append(line)
            if i < len(lines) - 1:
                pieces.append(b'\r\n' if any(start <= i < stop for start, stop in runs) else b'\n')
        out = b''.join(pieces)
    if sha(out) != edit['after_sha256']:
        raise SystemExit('edit postimage changed')
    return out


def package_files(package, archive):
    files = archive_files(archive, package)
    raw = (HERE / package['edits']['file']).read_bytes()
    if sha(raw) != package['edits']['sha256']:
        raise SystemExit(f"edit file checksum changed: {package['edits']['file']}")
    for edit in json.loads(raw)['edits']:
        name = edit['path']
        safe(name)
        if edit.get('added'):
            if name in files:
                raise SystemExit('added path already exists')
            value = edit['insert_utf8'].encode('utf-8')
            if sha(value) != edit['after_sha256']:
                raise SystemExit('added file checksum')
            files[name] = value
        else:
            files[name] = apply_edit(files[name], edit)
    if {n: sha(b) for n, b in files.items()} != package['postimage_files']:
        raise SystemExit(f"package postimage mismatch: {package['name']}")
    return files


def tree_hashes(root):
    return {p.relative_to(root).as_posix(): sha(p.read_bytes()) for p in root.rglob('*') if p.is_file()}


def write_package(base, package, files):
    out = base / crate_dir(package)
    tmp = base / f'.{crate_dir(package)}.tmp-{os.getpid()}'
    if tmp.exists():
        shutil.rmtree(tmp)
    for name, data in files.items():
        target = tmp / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    os.replace(tmp, out)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--repo', type=Path, default=HERE.parent, help='workspace root (default: this checkout)')
    parser.add_argument('--cache', type=Path, help='directory holding the pinned .crate archives')
    parser.add_argument('--download', action='store_true', help='fetch missing archives from static.crates.io')
    parser.add_argument('--verify', action='store_true', help='replay the archives and compare existing directories')
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_bytes())
    if manifest['schema'] != SCHEMA:
        raise SystemExit('dependencies.json schema')
    toml = (args.repo / 'Cargo.toml').read_text(encoding='utf-8')
    base = args.repo / '.patched'
    base.mkdir(exist_ok=True)
    (base / '.gitignore').write_text('*\n', encoding='utf-8')
    dirs = archive_dirs(args.cache)
    report = {}
    for package in manifest['packages']:
        name = crate_dir(package)
        if f'path = ".patched/{name}"' not in toml:
            raise SystemExit(f'Cargo.toml [patch.crates-io] does not name .patched/{name}')
        root = base / name
        if root.is_dir() and not args.verify:
            report[name] = 'present'
            continue
        files = package_files(package, find_archive(package, dirs, args.download))
        if root.is_dir():
            if tree_hashes(root) != {n: sha(b) for n, b in files.items()}:
                raise SystemExit(f'{root} differs from the pinned postimage; delete it and run again')
            report[name] = 'verified'
        else:
            write_package(base, package, files)
            report[name] = 'created'
    print(json.dumps(report, indent=1))


if __name__ == '__main__':
    main()
