#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: site_check.sh --book FILE --api FILE --shard FILE [--shard FILE] [--prose FILE] [--data FILE]"
}

die() {
  echo "docs_site: $1" >&2
  exit 1
}

reject() {
  echo "site_check: $1" >&2
  usage >&2
  exit 2
}

book=""
api=""
shards=()
prose=()
data=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    --book | --api | --shard | --prose | --data)
      [[ $# -ge 2 ]] || reject "$1 needs a FILE"
      case "$1" in
        --book) book="$2" ;;
        --api) api="$2" ;;
        --shard) shards+=("$2") ;;
        --prose) prose+=("$2") ;;
        --data) data+=("$2") ;;
      esac
      ;;
    *) reject "unknown argument '$1'" ;;
  esac
  shift 2
done

[[ -n "$book" ]] || reject "--book FILE is required"
[[ -n "$api" ]] || reject "--api FILE is required"
[[ ${#shards[@]} -gt 0 ]] || reject "--shard FILE is required"
[[ ${#prose[@]} -gt 0 ]] || reject "--prose FILE is required"

LC_ALL=C grep -q '^title' "$book" || die "book has no title"

for _f in "${prose[@]}"; do
  LC_ALL=C grep -q '^# ' "$_f" || die "prose missing title '$_f'"
done

ids="$({ LC_ALL=C grep -h '^  id: ' "${shards[@]}" || true; } | sed 's/^  id: "//;s/"$//' | LC_ALL=C sort -u)"

[[ -n "$ids" ]] || die "shard names no symbols"

for _sid in $ids; do
  LC_ALL=C grep -qF "$_sid" "$api" || die "missing API page for $_sid"
done

if LC_ALL=C grep -q '\[[^]]*\]()' "${prose[@]}"; then
  die "empty link target"
fi

anchors="$(
  {
    LC_ALL=C grep -h '^#' "${prose[@]}" || true
    LC_ALL=C grep -h '^## ' "$api" || true
  } | sed 's/^#* *//' | tr '[:upper:]' '[:lower:]' |
    sed 's/[^a-z0-9 -]//g; s/^ *//; s/ *$//; s/ /-/g; s/--*/-/g' | LC_ALL=C sort -u
)"

api_paths="$(printf '%s\n' "$ids" | sed 's|:|/|g;s|^|api/|;s|$|.md|' | LC_ALL=C sort -u)"

prose_bases="$(for _f in "${prose[@]}"; do basename "$_f"; done | LC_ALL=C sort -u)"

data_bases=""
if [[ ${#data[@]} -gt 0 ]]; then
  data_bases="$(for _f in "${data[@]}"; do basename "$_f"; done | LC_ALL=C sort -u)"
fi

targets="$(
  {
    LC_ALL=C grep -h -o '\[[^]]*\]([^)]*)' "${prose[@]}" 2>/dev/null |
      sed -n 's/.*(\([^)]*\)).*/\1/p' |
      sed 's/^ *//;s/ *$//;s/^<//;s/>$//;s/^".*//;s/".*$//;s/^ *//;s/ *$//' |
      cut -d' ' -f1 || true
    LC_ALL=C grep -h '^[ ]*\[[^]]*\]:' "${prose[@]}" 2>/dev/null |
      sed 's/^[^:]*:[[:space:]]*//;s/^<//;s/>$//;s/^"//;s/"$//' |
      cut -d' ' -f1 || true
  } | LC_ALL=C sort -u
)"

for _t in $targets; do
  case "$_t" in
    *://* | mailto:*) continue ;;
    \#*)
      _frag="$(printf '%s' "$_t" | cut -c2-)"
      printf '%s\n' "$anchors" | LC_ALL=C grep -qxF "$_frag" || die "dangling anchor '$_t'"
      ;;
    *)
      _base="$(printf '%s' "$_t" | cut -d'#' -f1)"
      _frag="$(printf '%s' "$_t" | cut -s -d'#' -f2- || true)"
      _dir=0
      case "$_base" in
        */)
          _dir=1
          _base="$(printf '%s' "$_base" | sed 's|/$||')"
          ;;
      esac
      _n="$_base"
      while [[ "$_n" == ../* ]]; do
        _n="${_n#../}"
      done
      _n="${_n#./}"
      _found=0
      if [[ "$_dir" == 1 ]]; then
        for _p in "${prose[@]}"; do
          case "$_p" in
            "$_n/README.md" | */"$_n/README.md")
              _found=1
              break
              ;;
          esac
        done
      else
        for _p in "${prose[@]}" ${data[@]+"${data[@]}"} $api_paths; do
          if [[ "$_p" == "$_n" || "$_p" == */"$_n" ]]; then
            _found=1
            break
          fi
        done
        if [[ "$_found" == 0 && "$_n" != */* ]]; then
          for _b in $prose_bases $data_bases; do
            if [[ "$_b" == "$_n" ]]; then
              _found=1
              break
            fi
          done
        fi
        for _k in api.md SUMMARY.md; do
          if [[ "$_n" == "$_k" ]]; then
            _found=1
            break
          fi
        done
      fi
      [[ "$_found" == 1 ]] || die "dangling link '$_t'"
      if [[ -n "$_frag" ]]; then
        _norm="$(printf '%s' "$_frag" | tr '[:upper:]' '[:lower:]')"
        printf '%s\n' "$anchors" | LC_ALL=C grep -qxF "$_norm" || die "dangling fragment '$_t'"
      fi
      ;;
  esac
done