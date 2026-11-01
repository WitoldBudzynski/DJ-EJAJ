#!/usr/bin/env python3
"""Build the native app bundle and collect third-party license files."""
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess

root = Path(__file__).resolve().parents[1]
cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo")
subprocess.run([cargo, "build", "--workspace", "--release", "--locked"], cwd=root, check=True)
bundle = root / "output/DJ EJAJ.app"
contents = bundle / "Contents"
(contents / "MacOS").mkdir(parents=True, exist_ok=True)
(contents / "Resources/licenses").mkdir(parents=True, exist_ok=True)
shutil.copy2(root / "target/release/djejaj", contents / "MacOS/djejaj")
os.chmod(contents / "MacOS/djejaj", 0o755)
with (contents / "Info.plist").open("wb") as target:
    plistlib.dump({
        "CFBundleDevelopmentRegion": "pl",
        "CFBundleDisplayName": "DJ EJAJ",
        "CFBundleName": "DJ EJAJ",
        "CFBundleIdentifier": "pl.nt4.djejaj",
        "CFBundleExecutable": "djejaj",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": "0.1.0",
        "CFBundleVersion": "1",
        "LSMinimumSystemVersion": "15.0",
        "NSHighResolutionCapable": True,
        "NSHumanReadableCopyright": "DJ EJAJ · by NT4.pl",
    }, target)
metadata = json.loads(subprocess.check_output([cargo, "metadata", "--locked", "--format-version=1"], cwd=root))
lines = ["# Third-party dependencies", "", "Built with the following Rust packages. License texts are included in licenses/.", ""]
for package in sorted(metadata["packages"], key=lambda p: p["name"]):
    if package["id"] in metadata["workspace_members"]:
        continue
    name = f'{package["name"]}-{package["version"]}'
    lines.append(f'- {name}: {package.get("license") or "See package license"} · {package.get("repository") or "https://crates.io/crates/" + package["name"]}')
    source = Path(package["manifest_path"]).parent
    destination = contents / "Resources/licenses" / name
    license_files = [p for p in source.rglob("*") if p.is_file() and p.name.lower().startswith(("license", "licence", "copying", "notice"))]
    if license_files:
        destination.mkdir(exist_ok=True)
        for license_file in license_files:
            target = destination / license_file.relative_to(source)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(license_file, target)
(contents / "Resources/THIRD_PARTY_NOTICES.md").write_text("\n".join(lines) + "\n")
print(bundle)
