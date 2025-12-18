#!/usr/bin/env python3
import sys
import re
import subprocess
import os


def main():
    cargo_path = "Cargo.toml"

    # 1. READ VERSION FROM CARGO.TOML (Updated by cargo-release)
    with open(cargo_path, "r") as f:
        content = f.read()

    ver_match = re.search(
        r'^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"', content, re.MULTILINE
    )
    if not ver_match:
        print("Error: Could not find version in Cargo.toml")
        sys.exit(1)

    major, minor, patch = map(int, ver_match.groups())
    version_string = f"{major}.{minor}.{patch}"

    # 2. CALCULATE VERSION CODE
    # Logic: 0.3.2 -> 302
    version_code = major * 10000 + minor * 100 + patch
    print(f"🚀 Preparing release v{version_string} (Code: {version_code})")

    # 3. UPDATE VERSION_CODE IN CARGO.TOML
    new_content = re.sub(
        r"^version_code\s*=\s*\d+",
        f"version_code = {version_code}",
        content,
        flags=re.MULTILINE,
    )

    with open(cargo_path, "w") as f:
        f.write(new_content)
    print(f"✅ Updated version_code to {version_code} in Cargo.toml")

    # 4. UPDATE MAIN CHANGELOG.MD
    # We use --tag so git-cliff treats the current "unreleased" changes as belonging to this new version
    print("📝 Updating CHANGELOG.md...")
    subprocess.run(
        ["git-cliff", "--tag", version_string, "--output", "CHANGELOG.md"], check=True
    )

    # 5. GENERATE FASTLANE CHANGELOG
    # F-Droid expects: fastlane/metadata/android/en-US/changelogs/{version_code}.txt
    fastlane_dir = "fastlane/metadata/android/en-US/changelogs"
    os.makedirs(fastlane_dir, exist_ok=True)
    fastlane_file = os.path.join(fastlane_dir, f"{version_code}.txt")

    print(f"📝 Generating Fastlane changelog: {fastlane_file}")

    # Generate just the body for the current release
    changelog_body = subprocess.check_output(
        ["git-cliff", "--tag", version_string, "--unreleased", "--strip", "header"],
        text=True,
    )

    # Clean up the output for the app store (remove HTML comments used for sorting)
    # e.g. "<!-- 0 -->🚀 Features" -> "🚀 Features"
    clean_body = re.sub(r"<!-- \d+ -->", "", changelog_body).strip()

    with open(fastlane_file, "w") as f:
        f.write(clean_body)

    # 6. STAGE THE NEW FILE FOR GIT
    # This ensures cargo-release includes the new .txt file in the release commit
    subprocess.run(["git", "add", fastlane_file], check=True)


if __name__ == "__main__":
    main()
