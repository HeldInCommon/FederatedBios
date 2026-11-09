#!/usr/bin/env bash
# End-to-end federation test: stands up a node and a separate commons, then
# proves publish → sign → federate, tamper rejection, and the members-only guard.
# Usage:  ./scripts/test-federation.sh     (builds the release binary if needed)
set -u
cd "$(dirname "$0")/.."
BIN="commons/target/release/hic-commons"
[ -x "$BIN" ] || cargo build --release --manifest-path commons/Cargo.toml
T="$(mktemp -d)"; mkdir -p "$T/commons" "$T/node"
PASS=0; FAIL=0
ok(){ echo "  PASS  $1"; PASS=$((PASS+1)); }
no(){ echo "  FAIL  $1"; FAIL=$((FAIL+1)); }

HIC_DATA="$T/commons" PORT=8091 HIC_ROLE=commons HIC_INSTANCE="Shared Commons" HIC_BASE_URL=http://localhost:8091 "$BIN" >"$T/commons.log" 2>&1 & CPID=$!
HIC_DATA="$T/node" PORT=8090 HIC_INSTANCE="Tega node" HIC_BASE_URL=http://localhost:8090 HIC_SHARE_DEFAULT=http://localhost:8091 "$BIN" >"$T/node.log" 2>&1 & NPID=$!
cleanup(){ kill $CPID $NPID ${IPID:-} ${CAPPID:-} ${C2PID:-} ${N3PID:-} 2>/dev/null; rm -rf "$T"; }
trap cleanup EXIT

curl -sf --retry 40 --retry-delay 1 --retry-connrefused http://localhost:8091/health >/dev/null && ok "commons up" || no "commons failed to start"
curl -sf --retry 40 --retry-delay 1 --retry-connrefused http://localhost:8090/health >/dev/null && ok "node up" || no "node failed to start"

SID=$(curl -is -X POST http://localhost:8090/join --data-urlencode "handle=tega" --data-urlencode "display_name=Tega Brain" --data-urlencode "passphrase=supersecret1" | tr -d '\r' | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
[ -n "$SID" ] && ok "created @tega + session" || no "no session from /join"
CSRF=$(curl -s http://localhost:8090/new -H "Cookie: hic_session=$SID" | grep -oE 'name="csrf" value="[0-9a-f]+"' | head -1 | grep -oE '[0-9a-f]{8,}')
[ -n "$CSRF" ] && ok "got CSRF token" || no "no CSRF token"

PUB=$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8090/publish -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "name=Tega Brain" --data-urlencode "bio=Artist and researcher working on environmental data." --data-urlencode "visibility=public" --data-urlencode "license=cc-by" --data-urlencode "signals=credit")
[ "$PUB" = "303" ] && ok "published public profile" || no "publish returned $PUB"

curl -s -X POST http://localhost:8090/share -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8091" | grep -qi "federated to" && ok "node federated a signed copy" || no "share failed"
grep -rq 'tega' "$T/commons/remote/" 2>/dev/null && ok "commons stored verified remote @tega" || no "no remote entry on commons"
curl -s http://localhost:8091/ | grep -qi "tega" && ok "@tega in commons directory" || no "@tega not in directory"
curl -s http://localhost:8091/api/directory | grep -q "tega" && no "tega wrongly local on commons" || ok "tega federated, not local"

# The commons is a federation target, not an identity store: /join and /login are
# gated (HIC_ROLE=commons).
curl -s http://localhost:8091/join | grep -qi "shared commons" && ok "commons /join shows the gate page" || no "commons /join not gated"
curl -s http://localhost:8091/login | grep -qi "shared commons" && ok "commons /login shows the gate page" || no "commons /login not gated"
GSID=$(curl -is -X POST http://localhost:8091/join --data-urlencode "handle=sneak" --data-urlencode "display_name=Sneak" --data-urlencode "passphrase=supersecret9" | tr -d '\r' | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
[ -z "$GSID" ] && ok "commons /join mints no identity" || no "commons /join created a session"
# ...while the node still allows normal sign-up (proven by @tega above).
curl -s http://localhost:8090/join | grep -qi "join the commons" && ok "node /join still allows sign-up" || no "node /join wrongly gated"

# The role is saved with the data (instance.json) and is fixed for that folder.
grep -q '"role": "commons"' "$T/commons/instance.json" 2>/dev/null && ok "commons saved its role in instance.json" || no "no commons role in instance.json"
grep -q '"role": "node"' "$T/node/instance.json" 2>/dev/null && ok "node saved its role in instance.json" || no "no node role in instance.json"
# refuses VAR=… : start the binary; succeed if it exits on its own within ~5s.
refuses(){ env "$@" PORT=8095 "$BIN" >"$T/refuse.log" 2>&1 & local p=$! i=0
  while kill -0 $p 2>/dev/null && [ $i -lt 50 ]; do sleep 0.1; i=$((i+1)); done
  if kill -0 $p 2>/dev/null; then kill $p; wait $p 2>/dev/null; return 1; fi; return 0; }
refuses HIC_DATA="$T/commons" HIC_ROLE=node && grep -q "belongs to a commons" "$T/refuse.log" && ok "commons folder refuses to start as a node" || no "commons folder started as a node"
cp -R "$T/node" "$T/node-copy" && rm -f "$T/node-copy/instance.json"
refuses HIC_DATA="$T/node-copy" HIC_ROLE=commons && grep -q "member identities" "$T/refuse.log" && ok "older node folder refuses to become a commons" || no "node folder became a commons"
mkdir -p "$T/legacy"
refuses HIC_DATA="$T/legacy" HIC_INSTANCE="Held in Common (federated)"
grep -q '"role": "commons"' "$T/legacy/instance.json" 2>/dev/null && ok "legacy commons label still starts a commons" || no "legacy commons guess lost"
curl -s http://localhost:8091/ | grep -q "Nodes share to" && ok "commons banner shows the address to share to" || no "commons banner missing share address"
# Handle pinning: a second node that also calls itself @tega (new key, different
# instance label) must not be able to take over @tega on the commons. The pin is
# on the handle alone, because origin and origin label are chosen by the sender.
mkdir -p "$T/impostor"
HIC_DATA="$T/impostor" PORT=8092 HIC_INSTANCE="Impostor node" HIC_BASE_URL=http://localhost:8092 "$BIN" >"$T/impostor.log" 2>&1 & IPID=$!
curl -sf --retry 40 --retry-delay 1 --retry-connrefused http://localhost:8092/health >/dev/null || no "impostor node failed to start"
ISID=$(curl -is -X POST http://localhost:8092/join --data-urlencode "handle=tega" --data-urlencode "display_name=Tega Brain" --data-urlencode "passphrase=supersecret3" | tr -d '\r' | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
ICSRF=$(curl -s http://localhost:8092/new -H "Cookie: hic_session=$ISID" | grep -oE 'name="csrf" value="[0-9a-f]+"' | head -1 | grep -oE '[0-9a-f]{8,}')
curl -s -o /dev/null -X POST http://localhost:8092/publish -H "Cookie: hic_session=$ISID" --data-urlencode "handle=tega" --data-urlencode "csrf=$ICSRF" --data-urlencode "name=Tega Brain" --data-urlencode "bio=Definitely the real one." --data-urlencode "visibility=public" --data-urlencode "license=cc-by"
curl -s -X POST http://localhost:8092/share -H "Cookie: hic_session=$ISID" --data-urlencode "handle=tega" --data-urlencode "csrf=$ICSRF" --data-urlencode "commons_url=http://localhost:8091" | grep -q "already used on localhost:8091" && ok "same handle from another node + key refused (409)" || no "impostor @tega was not refused"
grep -rq "Definitely the real one" "$T/commons/remote/" 2>/dev/null && no "impostor profile stored on the commons" || ok "impostor profile not stored"
curl -s -X POST http://localhost:8090/share -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8091" | grep -qi "federated to" && ok "original @tega can still re-share (same key)" || no "original @tega re-share refused"

# Handles are unique per commons, not globally.
curl -s http://localhost:8091/api/handle/tega | grep -q '"available":false' && ok "commons reports @tega taken" || no "api/handle did not report @tega taken"
curl -s http://localhost:8091/api/handle/nobody_yet | grep -q '"available":true' && ok "commons reports a free handle available" || no "api/handle did not report a free handle"
# A node whose default commons already has @tega refuses that handle at sign-up.
mkdir -p "$T/node3"
HIC_DATA="$T/node3" PORT=8093 HIC_ROLE=node HIC_INSTANCE="Third node" HIC_BASE_URL=http://localhost:8093 HIC_SHARE_DEFAULT=http://localhost:8091 "$BIN" >"$T/node3.log" 2>&1 & N3PID=$!
curl -sf --retry 40 --retry-delay 1 --retry-connrefused http://localhost:8093/health >/dev/null || no "third node failed to start"
curl -s -X POST http://localhost:8093/join --data-urlencode "handle=tega" --data-urlencode "display_name=Tega" --data-urlencode "passphrase=supersecret4" | grep -q "already used on localhost:8091" && ok "sign-up refused when default commons has the handle" || no "sign-up allowed a handle taken on the default commons"
[ -e "$T/node3/handles/tega.json" ] && no "refused sign-up still minted @tega" || ok "refused sign-up minted no identity"
# ...but the same handle may live on a different commons (one key per handle per commons).
mkdir -p "$T/commons2"
HIC_DATA="$T/commons2" PORT=8094 HIC_ROLE=commons HIC_INSTANCE="Second commons" HIC_BASE_URL=http://localhost:8094 "$BIN" >"$T/commons2.log" 2>&1 & C2PID=$!
curl -sf --retry 40 --retry-delay 1 --retry-connrefused http://localhost:8094/health >/dev/null || no "second commons failed to start"
curl -s -X POST http://localhost:8092/share -H "Cookie: hic_session=$ISID" --data-urlencode "handle=tega" --data-urlencode "csrf=$ICSRF" --data-urlencode "commons_url=http://localhost:8094" | grep -qi "federated to" && ok "other @tega accepted on a different commons" || no "per-commons uniqueness wrongly global"
# The origin is shown with the card: node address + key fingerprint.
curl -s http://localhost:8091/ | grep -q 'from localhost:8090' && ok "commons card shows origin address" || no "card missing origin address"
curl -s http://localhost:8091/ | grep -q 'class="dir-key"' && ok "commons card shows key fingerprint" || no "card missing key fingerprint"

python3 - "$T/cap.json" <<'PY' & 
import http.server,sys
out=sys.argv[1]
class H(http.server.BaseHTTPRequestHandler):
  def do_POST(s):
    n=int(s.headers.get('Content-Length',0)); b=s.rfile.read(n); open(out,'wb').write(b)
    s.send_response(200); s.end_headers(); s.wfile.write(b'{"ok":1}')
  def log_message(s,*a): pass
http.server.HTTPServer(('127.0.0.1',8099),H).serve_forever()
PY
CAPPID=$!
n=0; until (exec 3<>/dev/tcp/127.0.0.1/8099) 2>/dev/null; do n=$((n+1)); [ $n -gt 400 ] && break; done; exec 3>&- 2>/dev/null
curl -s -o /dev/null -X POST http://localhost:8090/share -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8099"
[ -s "$T/cap.json" ] && ok "captured on-the-wire signed activity" || no "capture failed"

[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8091/inbox -H 'Content-Type: application/json' --data-binary @"$T/cap.json")" = "202" ] && ok "valid activity accepted (202)" || no "valid replay not 202"
sed 's/environmental/TAMPERED/' "$T/cap.json" > "$T/tamper.json"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8091/inbox -H 'Content-Type: application/json' --data-binary @"$T/tamper.json")" = "400" ] && ok "tampered content rejected (400)" || no "tamper not 400"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8091/inbox -H 'Content-Type: application/json' --data-binary 'not json')" = "400" ] && ok "garbage rejected (400)" || no "garbage not 400"

SID2=$(curl -is -X POST http://localhost:8090/join --data-urlencode "handle=mo" --data-urlencode "display_name=Mo Secret" --data-urlencode "passphrase=supersecret2" | tr -d '\r' | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
CSRF2=$(curl -s http://localhost:8090/new -H "Cookie: hic_session=$SID2" | grep -oE 'name="csrf" value="[0-9a-f]+"' | head -1 | grep -oE '[0-9a-f]{8,}')
curl -s -o /dev/null -X POST http://localhost:8090/publish -H "Cookie: hic_session=$SID2" --data-urlencode "handle=mo" --data-urlencode "csrf=$CSRF2" --data-urlencode "name=Mo Secret" --data-urlencode "bio=Private." --data-urlencode "visibility=members" --data-urlencode "license=cc-by"
curl -s -X POST http://localhost:8090/share -H "Cookie: hic_session=$SID2" --data-urlencode "handle=mo" --data-urlencode "csrf=$CSRF2" --data-urlencode "commons_url=http://localhost:8091" | grep -qi "never shared\|stay on your node" && ok "members-only share blocked on node" || no "members-only not blocked"
curl -s http://localhost:8091/ | grep -qi 'Mo Secret\|@mo' && no "members handle leaked to commons" || ok "members handle never reached commons"

echo ""; echo "RESULT: $PASS passed, $FAIL failed"
exit $FAIL
