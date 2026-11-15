#!/usr/bin/env bash
set -euo pipefail

dir=${AGSYNC_PACKAGING_DIR:-packaging}
scoop=https://raw.githubusercontent.com/ScoopInstaller/Scoop/master/schema.json
winget=https://raw.githubusercontent.com/microsoft/winget-cli/master/schemas/JSON/manifests/v1.6.0
check=${CHECK_JSONSCHEMA:-uvx check-jsonschema==0.33.0}

$check --schemafile "$scoop" "$dir/scoop/agsync.json"
$check --schemafile "$winget/manifest.version.1.6.0.json" "$dir/winget/Chamoouske.agsync.yaml"
$check --schemafile "$winget/manifest.installer.1.6.0.json" "$dir/winget/Chamoouske.agsync.installer.yaml"
$check --schemafile "$winget/manifest.defaultLocale.1.6.0.json" "$dir/winget/Chamoouske.agsync.locale.en-US.yaml"
