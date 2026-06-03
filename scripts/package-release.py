#!/usr/bin/env python3
"""Package a tb2d release binary with README and workspace templates.

Creates both .tar.gz and .zip archives in the requested output directory.
"""

from __future__ import annotations

import argparse
import shutil
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path


def copy_into(staging_dir: Path, source: Path, destination_name: str) -> None:
    destination = staging_dir / destination_name
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def add_directory_to_zip(zip_file: zipfile.ZipFile, root: Path, staging_root: Path) -> None:
    for path in sorted(root.rglob("*")):
        arcname = path.relative_to(staging_root).as_posix()
        if path.is_dir():
            zip_info = zipfile.ZipInfo(f"{arcname}/")
            zip_file.writestr(zip_info, b"")
        else:
            zip_file.write(path, arcname)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path, help="Path to the built tb2d binary")
    parser.add_argument("--readme", default=Path("README.md"), type=Path, help="Path to README.md")
    parser.add_argument("--license", default=Path("LICENSE"), type=Path, help="Path to LICENSE")
    parser.add_argument("--changelog", default=Path("CHANGELOG.md"), type=Path, help="Path to CHANGELOG.md")
    parser.add_argument(
        "--default-config",
        default=Path("examples/default.yaml"),
        type=Path,
        help="Path to the default workspace config",
    )
    parser.add_argument(
        "--example-config",
        default=Path("examples/web-reader.yaml"),
        type=Path,
        help="Path to the example workspace config",
    )
    parser.add_argument("--out-dir", required=True, type=Path, help="Directory for release archives")
    parser.add_argument("--version", required=True, help="Release version/tag string")
    parser.add_argument("--platform", required=True, help="Platform identifier used in archive names")
    parser.add_argument("--name", default="tb2d", help="Binary/package name")
    args = parser.parse_args()

    binary = args.binary
    readme = args.readme
    license_file = args.license
    changelog = args.changelog
    default_config = args.default_config
    example_config = args.example_config

    for path in (binary, readme, license_file, changelog, default_config, example_config):
        if not path.exists():
            print(f"error: missing required input: {path}", file=sys.stderr)
            return 1

    args.out_dir.mkdir(parents=True, exist_ok=True)
    archive_base = f"{args.name}-{args.version}-{args.platform}"

    with tempfile.TemporaryDirectory(prefix=f"{archive_base}-stage-") as tmp:
        staging_root = Path(tmp)
        package_root = staging_root / archive_base
        package_root.mkdir(parents=True)

        copy_into(package_root, binary, args.name)
        copy_into(package_root, readme, "README.md")
        copy_into(package_root, license_file, "LICENSE")
        copy_into(package_root, changelog, "CHANGELOG.md")
        copy_into(package_root, default_config, default_config.name)
        copy_into(package_root, example_config, example_config.name)

        tar_path = args.out_dir / f"{archive_base}.tar.gz"
        zip_path = args.out_dir / f"{archive_base}.zip"

        with tarfile.open(tar_path, mode="w:gz") as tar:
            tar.add(package_root, arcname=archive_base)

        with zipfile.ZipFile(zip_path, mode="w", compression=zipfile.ZIP_DEFLATED) as zf:
            add_directory_to_zip(zf, package_root, staging_root)

    print(f"created {tar_path}")
    print(f"created {zip_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
