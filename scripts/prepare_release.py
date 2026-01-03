#!/usr/bin/env python3
import datetime
import os
import re
import subprocess
import sys


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
    print("📝 Updating CHANGELOG.md...")
    subprocess.run(
        ["git-cliff", "--tag", version_string, "--output", "CHANGELOG.md"], check=True
    )

    # 5. GENERATE FASTLANE CHANGELOG
    fastlane_dir = "fastlane/metadata/android/en-US/changelogs"
    os.makedirs(fastlane_dir, exist_ok=True)
    fastlane_file = os.path.join(fastlane_dir, f"{version_code}.txt")

    print(f"📝 Generating Fastlane changelog: {fastlane_file}")
    changelog_body = subprocess.check_output(
        ["git-cliff", "--tag", version_string, "--unreleased", "--strip", "header"],
        text=True,
    )
    clean_body = re.sub(r"<!-- \d+ -->", "", changelog_body).strip()

    with open(fastlane_file, "w") as f:
        f.write(clean_body)

    # --- NEW FLATPAK STEPS ---

    # 6. GENERATE FLATPAK SOURCES (cargo-sources.yml)
    # Using the system-installed flatpak-cargo-generator
    print("📦 Generating Flatpak cargo sources (YAML)...")
    try:
        # We output to YAML because it's robust across flatpak-builder versions
        subprocess.run(
            ["flatpak-cargo-generator", "Cargo.lock", "-o", "cargo-sources.yml"],
            check=True,
        )
    except FileNotFoundError:
        print("Error: flatpak-cargo-generator not found in PATH.")
        sys.exit(1)

    # 7. UPDATE METAINFO.XML RELEASES
    metainfo_path = "assets/org.codeberg.trougnouf.cfait.metainfo.xml"
    print(f"📝 Updating Metainfo: {metainfo_path}")

    today = datetime.date.today().isoformat()
    release_tag = f'    <release version="{version_string}" date="{today}"/>'

    with open(metainfo_path, "r") as f:
        xml_content = f.read()

    # Insert the new release tag after <releases>
    # This regex looks for <releases> and appends our new tag right after it
    if "<releases>" in xml_content:
        xml_content = xml_content.replace("<releases>", f"<releases>\n{release_tag}")
    else:
        # Fallback if <releases> doesn't exist, insert before </component>
        xml_content = xml_content.replace(
            "</component>", f"  <releases>\n{release_tag}\n  </releases>\n</component>"
        )

    with open(metainfo_path, "w") as f:
        f.write(xml_content)

    # 8. UPDATE FLATPAK MANIFEST WITH NEW TAG AND COMMIT
    flatpak_manifest = "org.codeberg.trougnouf.cfait.yml"
    print(f"📝 Updating Flatpak manifest: {flatpak_manifest}")

    # Get the current commit hash (this assumes we're tagging the current HEAD)
    # Note: This runs before the actual git tag is created by cargo-release
    # So we get the commit that will be tagged
    try:
        commit_hash = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], text=True
        ).strip()
    except subprocess.CalledProcessError:
        print("Error: Could not get git commit hash")
        sys.exit(1)

    with open(flatpak_manifest, "r") as f:
        manifest_content = f.read()

    # Update the tag field
    manifest_content = re.sub(
        r"(\s+tag:\s+)v[\d.]+", rf"\1v{version_string}", manifest_content
    )

    # Update the commit field
    manifest_content = re.sub(
        r"(\s+commit:\s+)[a-f0-9]{40}", rf"\1{commit_hash}", manifest_content
    )

    with open(flatpak_manifest, "w") as f:
        f.write(manifest_content)

    print(
        f"✅ Updated Flatpak manifest to tag v{version_string}, commit {commit_hash[:8]}"
    )

    # 9. STAGE ALL FILES FOR GIT
    # cargo-release will commit these
    subprocess.run(
        [
            "git",
            "add",
            fastlane_file,
            "cargo-sources.yml",
            metainfo_path,
            flatpak_manifest,
            "Cargo.toml",
            "CHANGELOG.md",
        ],
        check=True,
    )


if __name__ == "__main__":
    main()
