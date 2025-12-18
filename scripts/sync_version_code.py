#!/usr/bin/env python3
import sys
import re

def main():
    cargo_path = "Cargo.toml"
    
    with open(cargo_path, "r") as f:
        content = f.read()

    # Find version = "x.y.z"
    ver_match = re.search(r'^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"', content, re.MULTILINE)
    if not ver_match:
        print("Error: Could not find version in Cargo.toml")
        sys.exit(1)

    major, minor, patch = map(int, ver_match.groups())
    
    # Calculate Code: 0.3.1 -> 301
    new_code = major * 10000 + minor * 100 + patch
    print(f"Syncing version code: {major}.{minor}.{patch} -> {new_code}")

    # Replace version_code = ...
    new_content = re.sub(
        r'^version_code\s*=\s*\d+',
        f'version_code = {new_code}',
        content,
        flags=re.MULTILINE
    )

    with open(cargo_path, "w") as f:
        f.write(new_content)

if __name__ == "__main__":
    main()