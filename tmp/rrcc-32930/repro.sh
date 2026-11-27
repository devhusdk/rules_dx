#!/usr/bin/env bash
# Repro for https://github.com/bazelbuild/bazel/issues/32930: with
# --experimental_remote_repo_contents_cache, a grpcs:// remote cache that stops answering
# blocks repository fetches on the cache lookup and the synchronous upload.
#
# Needs: bazel (bazelisk) on PATH, python3, openssl, curl; Linux amd64 for bazel-remote.
# Put repro.sh, endpoint.py, MODULE.bazel, BUILD and repos.bzl in one directory and run it.
#
# Settings (environment):
#   BAZEL_VERSION  Bazel to run through bazelisk (default 9.3.1; try last_green)
#   MODE           mid: a working cache (bazel-remote behind endpoint.py, which terminates TLS) that
#                       stops answering once the first repo starts fetching (default)
#                  start: an endpoint that never answers the TLS handshake
#   ON_STALL       for MODE=mid, what happens to connections that are already open:
#                  hang: they stay open and answer HTTP/2 PINGs, nothing else (default; like a proxy
#                        whose backend stopped answering)
#                  freeze: they stay open and nothing comes back, PINGs included
#                  cut: they are reset
#                  New connections never get a TLS ServerHello in any of these.
#   REPOS          number of repos (default 3); CHAIN=1 makes them fetch one after another
#   T, R           --remote_timeout and --remote_retries (defaults 60s and 5)
#   KEEP=1         keep the work directory; KEEP_SERVER=1 keeps the Bazel server running
# Extra arguments go to `bazel build`, e.g. --experimental_circuit_breaker_strategy=failure.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
BAZEL=${BAZEL:-bazel}
BAZEL_VERSION=${BAZEL_VERSION:-9.3.1}
MODE=${MODE:-mid}
ON_STALL=${ON_STALL:-hang}
REPOS=${REPOS:-3}
CHAIN=${CHAIN:-0}
T=${T:-60s}
R=${R:-5}

# The workspace and output base are reused across runs, so the Bazel server stays warm and its
# start-up is not timed. Each run gets a new nonce, so every repo misses the local and remote caches.
cache_root=${XDG_CACHE_HOME:-$HOME/.cache}/rrcc-repro
ws=$cache_root/$BAZEL_VERSION/ws
output_base=$cache_root/$BAZEL_VERSION/output_base
work=$(mktemp -d "${TMPDIR:-/tmp}/rrcc-repro.XXXXXX")
nonce=$(basename "$work")
mkdir -p "$ws" "$cache_root/bin"
cp "$here/MODULE.bazel" "$here/BUILD" "$here/repos.bzl" "$ws/"

# No rc files, so a remote cache, disk cache or headers configured elsewhere don't get in.
startup=(--nohome_rc --nosystem_rc --noworkspace_rc
  --output_user_root="$cache_root/user_root" --output_base="$output_base"
  --host_jvm_args=-Xmx1g --experimental_remote_repo_contents_cache)
bzl() { (cd "$ws" && USE_BAZEL_VERSION=$BAZEL_VERSION "$BAZEL" "${startup[@]}" "$@"); }

pids=()
cleanup() {
  [[ ${#pids[@]} -gt 0 ]] && kill "${pids[@]}" 2>/dev/null || true
  [[ ${KEEP_SERVER:-0} == 1 ]] || bzl shutdown 2>/dev/null || true
  if [[ ${KEEP:-0} == 1 ]]; then echo "work directory kept: $work"; else rm -rf "$work"; fi
}
trap cleanup EXIT

free_port() { python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])'; }
wait_for() {  # wait_for <description> <command...>
  local what=$1; shift
  for _ in $(seq 100); do "$@" 2>/dev/null && return; sleep 0.1; done
  echo "timed out waiting for $what" >&2; exit 1
}
port_open() { python3 -c 'import socket,sys; socket.create_connection(("127.0.0.1", int(sys.argv[1])), 1)' "$1"; }

# A self-signed certificate for 127.0.0.1, passed to Bazel with --tls_certificate.
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=127.0.0.1 \
  -addext subjectAltName=IP:127.0.0.1 -keyout "$work/key.pem" -out "$work/cert.pem" 2>/dev/null

port=$(free_port)
if [[ $MODE == mid ]]; then
  br=$cache_root/bin/bazel-remote-2.6.4
  if [[ ! -x $br ]]; then
    tmp=$br.$$.tmp  # per process, so two runs downloading at once don't collide
    curl -fsSL -o "$tmp" https://github.com/buchgr/bazel-remote/releases/download/v2.6.4/bazel-remote-2.6.4-linux-amd64
    echo "6a6b64666c2238ec6b49b7b29f5e41e4149de09417a0028897c9bffe40376876  $tmp" | sha256sum -c --quiet
    chmod +x "$tmp" && mv "$tmp" "$br"
  fi
  up=$(free_port)
  "$br" --dir "$work/remote" --max_size 1 --grpc_address "127.0.0.1:$up" --http_address "127.0.0.1:$(free_port)" \
    > "$work/bazel-remote.log" 2>&1 &
  pids+=($!)
  wait_for bazel-remote port_open "$up"
  python3 "$here/endpoint.py" --port "$port" --upstream "127.0.0.1:$up" --on-stall "$ON_STALL" \
    --cert "$work/cert.pem" --key "$work/key.pem" 2> "$work/endpoint.log" &
else
  python3 "$here/endpoint.py" --port "$port" 2> "$work/endpoint.log" &
fi
endpoint=$!
pids+=($endpoint)
# Wait for the log line, not the port: a probe connection would count as a handshake attempt.
wait_for endpoint.py grep -q listening "$work/endpoint.log"

mode=$MODE
[[ $MODE == mid ]] && mode+=" ON_STALL=$ON_STALL"
echo "== Bazel $BAZEL_VERSION, MODE=$mode REPOS=$REPOS CHAIN=$CHAIN T=$T R=$R extra: $*"
bzl info release > /dev/null 2>&1  # starts the server outside the timed part

if [[ $MODE == mid ]]; then
  # Stall once the first repo has started fetching: its cache lookup is done, its upload is not.
  ( until grep -qs "$nonce" "$output_base/external/+repos+r0/data.txt"; do sleep 0.2; done
    kill -USR1 "$endpoint" ) &
  pids+=($!)
fi

start=$(date +%s.%N)
rc=0
bzl build --nobuild //:deps \
  --remote_cache="grpcs://127.0.0.1:$port" --tls_certificate="$work/cert.pem" \
  --remote_timeout="$T" --remote_retries="$R" \
  --repo_contents_cache="$work/repo_contents_cache" --repository_cache="$cache_root/repository_cache" \
  --repo_env=REPRO_REPOS="$REPOS" --repo_env=REPRO_CHAIN="$CHAIN" --repo_env=REPRO_NONCE="$nonce" \
  --profile="$work/profile.json.gz" --remote_grpc_log="$work/grpc.log" \
  "$@" > "$work/bazel.log" 2>&1 || rc=$?
end=$(date +%s.%N)
echo "== exit code $rc, wall time $(python3 -c "print(f'{$end - $start:.1f}s')")"

python3 -I - "$work" <<'EOF'
import collections, gzip, json, os, re, sys

work = sys.argv[1]

# Per-repo "Fetching repository" events from the profile. Each one wraps one call of
# RepositoryFetchFunction.compute, i.e. the remote cache lookup, the fetch and the upload; a repo
# that waits for another one (CHAIN=1) shows up more than once. A repo rule here takes 2s, so a
# longest event well above that is time blocked on the remote cache.
ev = json.load(gzip.open(os.path.join(work, "profile.json.gz")))["traceEvents"]
t0 = min(e["ts"] for e in ev if "ts" in e)
fetch = collections.defaultdict(list)
for e in ev:
    name = e.get("name", "")
    if e.get("cat") == "Fetching repository" and "+repos+r" in name:
        s = (e["ts"] - t0) / 1e6
        fetch[name].append((s, s + e.get("dur", 0) / 1e6))
print("== repository fetches (profile; seconds from the start of the profile)")
print(f"  {'repo':<16} {'events':>6} {'first':>7} {'done':>7} {'longest':>8}")
for repo, spans in sorted(fetch.items()):
    longest = max(e - s for s, e in spans)
    print(f"  {repo:<16} {len(spans):6d} {min(s for s, _ in spans):7.1f} "
          f"{max(e for _, e in spans):7.1f} {longest:8.1f}")

# The gRPC log is a stream of length-delimited LogEntry messages
# (src/main/protobuf/remote_execution_log.proto); decode the few fields we need by hand.
def varint(b, i):
    x = s = 0
    while True:
        c = b[i]; i += 1; x |= (c & 0x7F) << s; s += 7
        if c < 0x80:
            return x, i

def fields(b):
    i, out = 0, collections.defaultdict(list)
    while i < len(b):
        key, i = varint(b, i)
        if key & 7 == 0:
            v, i = varint(b, i)
        elif key & 7 == 2:
            n, i = varint(b, i); v = b[i:i + n]; i += n
        elif key & 7 == 1:
            v = b[i:i + 8]; i += 8
        else:
            v = b[i:i + 4]; i += 4
        out[key >> 3].append(v)
    return out

def ts(b):
    f = fields(b)
    return f.get(1, [0])[0] + f.get(2, [0])[0] / 1e9

CODES = "OK CANCELLED UNKNOWN INVALID_ARGUMENT DEADLINE_EXCEEDED NOT_FOUND ALREADY_EXISTS " \
        "PERMISSION_DENIED RESOURCE_EXHAUSTED FAILED_PRECONDITION ABORTED OUT_OF_RANGE " \
        "UNIMPLEMENTED INTERNAL UNAVAILABLE DATA_LOSS UNAUTHENTICATED".split()
calls = collections.defaultdict(list)
msgs = collections.defaultdict(collections.Counter)
failed = []
path = os.path.join(work, "grpc.log")
b = open(path, "rb").read() if os.path.exists(path) else b""
i = 0
while i < len(b):
    n, i = varint(b, i)
    f = fields(b[i:i + n]); i += n
    if not {3, 5, 6} <= f.keys():
        continue  # not an RPC (newer Bazel also logs other entries)
    st = fields(f[2][0]) if 2 in f else {}
    code = CODES[st.get(1, [0])[0]]
    start, took = ts(f[5][0]), ts(f[6][0]) - ts(f[5][0])
    calls[code].append(took)
    if code != "OK":
        repo = fields(f[1][0]).get(2, [b""])[0].decode() if 1 in f else ""
        failed.append((start, took, code, f[3][0].decode().rsplit("/", 1)[-1], repo))
        if 2 in st:
            msg = st[2][0].decode(errors="replace").splitlines()[0]
            msgs[code][re.sub(r"\b[0-9a-f]{64}\b", "<digest>", msg)] += 1
print("== gRPC calls (--remote_grpc_log), by status code")
if not calls:
    print("  none logged")
for code, t in sorted(calls.items()):
    print(f"  {code:<18} calls={len(t):4d}  longest={max(t):7.1f}s  total={sum(t):8.1f}s")
    for m, c in msgs[code].most_common(3):
        print(f"    {c:3d} x {m[:110]}")
slow = [c for c in sorted(failed) if c[1] >= 5]
if slow:
    print("== failed gRPC calls that took 5s or more (start is seconds from the first failed call)")
    first = min(start for start, *_ in failed)
    for start, took, code, method, repo in slow[:15]:
        print(f"  start={start - first:7.1f}  took={took:6.1f}s  {code:<18} {method:<18} {repo}")

log = open(os.path.join(work, "endpoint.log")).read()
print(f"== endpoint: {log.count('accepted, stalling')} TLS handshake(s) left unanswered, "
      f"{log.count('answered PING')} PING(s) answered while stalling")
lines = open(os.path.join(work, "bazel.log")).read().splitlines()
warn = [l for l in lines if l.startswith("WARNING:") and "cache" in l]
print(f"== Bazel warnings about the remote cache: {len(warn)}")
for l in warn[:6]:
    print("  " + l.split(": java.")[0][9:].strip()[:110])
for l in [l for l in lines if l.startswith("ERROR:")][:3]:
    print("  " + l.strip()[:140])
EOF
