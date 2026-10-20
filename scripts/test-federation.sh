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

HIC_DATA="$T/commons" PORT=8091 HIC_INSTANCE="Shared Commons" HIC_BASE_URL=http://localhost:8091 "$BIN" >"$T/commons.log" 2>&1 & CPID=$!
HIC_DATA="$T/node" PORT=8090 HIC_INSTANCE="Tega node" HIC_BASE_URL=http://localhost:8090 HIC_SHARE_DEFAULT=http://localhost:8091 "$BIN" >"$T/node.log" 2>&1 & NPID=$!
cleanup(){ kill $CPID $NPID ${CAPPID:-} 2>/dev/null; rm -rf "$T"; }
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
