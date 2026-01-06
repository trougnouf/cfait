#!/usr/bin/env python3
import datetime
import os
import re
import subprocess
import sys


def main():
    cargo_path = "Cargo.toml"

    # 1. READ VERSION FROM CARGO.TOML
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

    # 6. UPDATE METAINFO.XML RELEASES
    metainfo_path = "assets/com.trougnouf.Cfait.metainfo.xml"
    print(f"📝 Updating Metainfo: {metainfo_path}")

    today = datetime.date.today().isoformat()
    release_tag = f'    <release version="{version_string}" date="{today}"/>'

    with open(metainfo_path, "r") as f:
        xml_content = f.read()

    if "<releases>" in xml_content:
        xml_content = xml_content.replace("<releases>", f"<releases>\n{release_tag}")
    else:
        xml_content = xml_content.replace(
            "</component>", f"  <releases>\n{release_tag}\n  </releases>\n</component>"
        )

    with open(metainfo_path, "w") as f:
        f.write(xml_content)

    # 7. UPDATE LOCAL FLATPAK MANIFEST
    # We update this so the file in the repo remains valid for local testing,
    # even though the CI handles the actual Flathub update.
    flatpak_manifest = "com.trougnouf.Cfait.yml"
    if os.path.exists(flatpak_manifest):
        print(f"📝 Updating local Flatpak manifest: {flatpak_manifest}")

        try:
            commit_hash = subprocess.check_output(
                ["git", "rev-parse", "HEAD"], text=True
            ).strip()
        except subprocess.CalledProcessError:
            print("Warning: Could not get git commit hash, skipping manifest update")
            commit_hash = None

        if commit_hash:
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

    # 8. STAGE ALL FILES FOR GIT
    files_to_add = [
        fastlane_file,
        metainfo_path,
        "Cargo.toml",
        "CHANGELOG.md",
        # We add the manifest if it exists, so the version bump is committed
        flatpak_manifest if os.path.exists(flatpak_manifest) else None,
    ]
    
    # Filter out None values
    files_to_add = [f for f in files_to_add if f]

    subprocess.run(
        ["git", "add"] + files_to_add,
        check=True,
    )


if __name__ == "__main__":
    main()
