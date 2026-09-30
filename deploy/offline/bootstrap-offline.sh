#!/usr/bin/env bash
set -euo pipefail

bundle=""
install_dir="${HOME:-/tmp}/.local/bin"
workspace="$PWD"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bundle)
      bundle="${2:-}"
      shift 2
      ;;
    --install-dir)
      install_dir="${2:-}"
      shift 2
      ;;
    --workspace)
      workspace="${2:-}"
      shift 2
      ;;
    -h | --help)
      echo "usage: bootstrap-offline.sh --bundle DIR [--install-dir DIR] [--workspace DIR]"
      exit 0
      ;;
    *)
      echo "bootstrap-offline: unknown argument '$1'" >&2
      echo "usage: bootstrap-offline.sh --bundle DIR [--install-dir DIR] [--workspace DIR]" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$bundle" ]]; then
  echo "bootstrap-offline: missing --bundle DIR (vendored offline bundle)" >&2
  exit 1
fi
if [[ ! -d "$bundle/bazelisk" ]]; then
  echo "bootstrap-offline: bundle has no bazelisk/ dir: $bundle" >&2
  exit 1
fi
if [[ ! -d "$bundle/advisory" ]]; then
  echo "bootstrap-offline: bundle has no advisory/ dir: $bundle" >&2
  exit 1
fi

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) asset=bazelisk-linux-amd64 ;;
  Linux-aarch64 | Linux-arm64) asset=bazelisk-linux-arm64 ;;
  Darwin-x86_64) asset=bazelisk-darwin-amd64 ;;
  Darwin-arm64) asset=bazelisk-darwin-arm64 ;;
  MINGW*-x86_64 | MSYS*-x86_64 | CYGWIN*-x86_64) asset=bazelisk-windows-amd64.exe ;;
  *)
    echo "bootstrap-offline: unsupported host $(uname -s)-$(uname -m)" >&2
    exit 1
    ;;
esac

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d' ' -f1
  else
    python3 -c 'import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$1"
  fi
}

verify_manifest() {
  dir="$1"
  manifest="$dir/SHA256SUMS"
  if [[ ! -f "$manifest" ]]; then
    echo "bootstrap-offline: missing manifest: $manifest" >&2
    return 1
  fi
  while read -r want name; do
    [[ -z "$want" || "$want" == \#* ]] && continue
    if [[ -z "$name" || ! -f "$dir/$name" ]]; then
      echo "bootstrap-offline: manifest names missing file: ${name:-<empty>} (in $manifest)" >&2
      return 1
    fi
    got="$(hash_file "$dir/$name")"
    if [[ "$got" != "$want" ]]; then
      echo "bootstrap-offline: checksum mismatch for $name: got $got want $want" >&2
      return 1
    fi
  done <"$manifest"
}

today_utc() {
  date -u +%F
}

if [[ ! -f "$bundle/bazelisk/$asset" ]]; then
  echo "bootstrap-offline: bundle has no launcher for this host: $asset" >&2
  exit 1
fi
verify_manifest "$bundle/bazelisk"
verify_manifest "$bundle/advisory"
launcher_want="$(awk -v asset="$asset" '$2 == asset {print $1}' "$bundle/bazelisk/SHA256SUMS")"
if [[ -z "$launcher_want" ]]; then
  echo "bootstrap-offline: manifest does not pin this host launcher: $asset" >&2
  exit 1
fi
launcher_got="$(hash_file "$bundle/bazelisk/$asset")"
if [[ "$launcher_got" != "$launcher_want" ]]; then
  echo "bootstrap-offline: checksum mismatch for $asset: got $launcher_got want $launcher_want" >&2
  exit 1
fi

mkdir -p "$install_dir"
if [[ "$asset" == *.exe ]]; then
  cp -f "$bundle/bazelisk/$asset" "$install_dir/bazel.exe"
  chmod +x "$install_dir/bazel.exe"
  installed="$install_dir/bazel.exe"
else
  cp -f "$bundle/bazelisk/$asset" "$install_dir/bazel"
  chmod +x "$install_dir/bazel"
  installed="$install_dir/bazel"
fi

today="$(today_utc)"
mkdir -p "$workspace/.dx/advisory"
populated=0
for snapshot in "$bundle"/advisory/*.json; do
  [[ -e "$snapshot" ]] || continue
  set_name="$(basename "$snapshot" .json)"
  case "$set_name" in
    cargo | npm | maven | nuget | go | rubygems) ;;
    *)
      echo "bootstrap-offline: bundle carries unknown advisory set: $set_name" >&2
      exit 1
      ;;
  esac
  sha="$(hash_file "$snapshot")"
  cp -f "$snapshot" "$workspace/.dx/advisory/$set_name.json"
  cat >"$workspace/.dx/advisory/$set_name.meta.json" <<EOF
{"set": "$set_name", "url": "file://$bundle/advisory/$set_name.json", "sha256": "$sha", "retrieved_at": "$today", "path": ".dx/advisory/$set_name.json"}
EOF
  populated=$((populated + 1))
done
if [[ "$populated" -eq 0 ]]; then
  echo "bootstrap-offline: bundle carries no advisory snapshots" >&2
  exit 1
fi

echo "bootstrap-offline: installed $installed (sha256 $launcher_got, no network)"
echo "bootstrap-offline: populated $populated advisory snapshots under $workspace/.dx/advisory (retrieved_at $today)"
echo "bootstrap-offline: first Bazel module/toolchain fetch still needs network once; steady-state offline after"
