#!/usr/bin/env python3
"""Package an already-signed APK with its source project's license notices."""
import argparse
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--apk", type=Path, required=True)
parser.add_argument("--out", type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
if not args.apk.is_file():
    parser.error("APK does not exist")
if args.apk.resolve() == args.out.resolve():
    parser.error("Bundle output must differ from the signed APK")
args.out.parent.mkdir(parents=True, exist_ok=True)
with ZipFile(args.out, "w", compression=ZIP_DEFLATED, compresslevel=6) as bundle:
    bundle.write(args.apk, args.apk.name)
    for name in ("LICENSE-MIT", "LICENSE-APACHE", "NOTICE", "ATTRIBUTION.md"):
        bundle.write(root / name, "licenses/" + name)
    for notice in sorted((root / "packaging/android/licenses").iterdir()):
        if notice.is_file():
            bundle.write(notice, "licenses/" + notice.name)
print(args.out)
