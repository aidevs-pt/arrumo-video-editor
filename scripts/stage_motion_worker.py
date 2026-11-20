#!/usr/bin/env python3
"""Stage a verified SDK motion worker artifact for the Linux desktop package."""

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path, PurePosixPath
from zipfile import ZipFile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("sha256", help="Expected checksum from the trusted SDK build")
    args = parser.parse_args()
    data = args.archive.read_bytes()
    if sha256(data).hexdigest() != args.sha256.lower():
        parser.error("The SDK motion worker checksum does not match")
    destination = Path(__file__).resolve().parents[1] / "src-tauri/motion-worker"
    with ZipFile(BytesIO(data)) as archive:
        names = archive.namelist()
        if "tools/run_motion_package.py" not in names or len(names) != len(set(names)) or any(
            any(part in ("", ".", "..") for part in name.split("/"))
            or PurePosixPath(name).is_absolute() or "\\" in name
            for name in names
        ):
            parser.error("Invalid SDK worker archive layout")
        destination.mkdir()
        archive.extractall(destination)


if __name__ == "__main__":
    main()
