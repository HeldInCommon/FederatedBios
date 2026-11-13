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
HIC_DATA="$T/node" PORT=8090 HIC_INSTANCE="Tega node" HIC_BASE_URL=http://localhost:8090 HIC_SHARE_DEFAULT=http://localhost:8091 HIC_SHARE_DEFAULT_NAME="Test Collective" "$BIN" >"$T/node.log" 2>&1 & NPID=$!
cleanup(){ kill $CPID $NPID ${IPID:-} ${CAPPID:-} ${C2PID:-} ${N3PID:-} ${RPID:-} 2>/dev/null; rm -rf "$T"; }
trap cleanup EXIT

curl -sf --retry 40 --retry-delay 1 --retry-connrefused http://localhost:8091/health >/dev/null && ok "commons up" || no "commons failed to start"
# A new commons makes a one-time founding invite for its first member (ADR 0011).
FOUND1=$(grep -oE 'hic-founding-[0-9a-f]+' "$T/commons/founding-invite.json" 2>/dev/null)
[ -n "$FOUND1" ] && ok "commons made a founding invite" || no "no founding invite"
# The steward view (ADR 0017, 0023): only with the steward link the commons
# prints in its log. Being at the commons' computer isn't enough, since a
# proxy such as Tailscale Funnel makes every visitor look local.
grep -q "steward link (keep private): http://localhost:8091/steward?key=" "$T/commons.log" && ok "commons printed a steward link" || no "no steward link in the log"
SLINK=$(grep -oE 'http://localhost:8091/steward\?key=[0-9a-f]+' "$T/commons.log" | head -1)
curl -s -i http://localhost:8091/steward | tr -d '\r' | grep -qi '^set-cookie' && no "a bare /steward from this computer got a session" || ok "a bare /steward from this computer is refused"
curl -s http://localhost:8091/ | grep -q "Open the steward view" && no "visitors are still offered the steward view" || ok "visitors aren't offered the steward view"
STEWARD=$(curl -s -i "$SLINK" | tr -d '\r' | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
SPAGE=$(curl -s http://localhost:8091/ -H "Cookie: hic_session=$STEWARD")
printf '%s' "$SPAGE" | grep -q "hic-founding-" && ok "steward view shows the founding invite" || no "steward view missing the founding invite"
SCSRF=$(printf '%s' "$SPAGE" | grep -oE 'name="csrf" value="[0-9a-f]+"' | head -1 | grep -oE '[0-9a-f]{8,}')
curl -sf --retry 40 --retry-delay 1 --retry-connrefused http://localhost:8090/health >/dev/null && ok "node up" || no "node failed to start"

SIGNUP=$(curl -is -X POST http://localhost:8090/join --data-urlencode "handle=tega" --data-urlencode "display_name=Tega Brain" --data-urlencode "passphrase=supersecret1" --data-urlencode "passphrase_again=supersecret1" | tr -d '\r')
SID=$(printf '%s\n' "$SIGNUP" | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
# Sign-up shows the recovery code once (decision record 0010); keep it for later.
RCODE=$(printf '%s\n' "$SIGNUP" | grep -oE '[0-9a-f]{8}(-[0-9a-f]{8}){7}' | head -1)
[ -n "$RCODE" ] && ok "sign-up showed a recovery code" || no "no recovery code at sign-up"
[ -n "$SID" ] && ok "created @tega + session" || no "no session from /join"
CSRF=$(curl -s http://localhost:8090/new -H "Cookie: hic_session=$SID" | grep -oE 'name="csrf" value="[0-9a-f]+"' | head -1 | grep -oE '[0-9a-f]{8,}')
[ -n "$CSRF" ] && ok "got CSRF token" || no "no CSRF token"

PUB=$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8090/publish -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "name=Tega Brain" --data-urlencode "bio=Artist and researcher working on environmental data." --data-urlencode "visibility=public" --data-urlencode "license=cc-by" --data-urlencode "signals=credit" --data-urlencode "withdraw_after_training=yes" --data-urlencode "own_terms=Open research only. <b>Never</b> my voice.")
[ "$PUB" = "303" ] && ok "published public profile (with Held in Common Signals)" || no "publish returned $PUB"

curl -s -X POST http://localhost:8090/share -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8091" --data-urlencode "invite=$FOUND1" | grep -qi "federated to" && ok "node federated a signed copy (with the founding invite)" || no "share failed"
grep -rq 'tega' "$T/commons/remote/" 2>/dev/null && ok "commons stored verified remote @tega" || no "no remote entry on commons"
grep -rq '"schema": "hic/v1"' "$T/commons/remote/" 2>/dev/null && ok "share arrived in format hic/v1 (every field signed)" || no "share not in hic/v1"
# Held in Common Signals (decision record 0025) travel signed, as a ProfileV2,
# and the commons shows them, with the person's own words as plain text.
grep -rq '"type": "ProfileV2"' "$T/commons/remote/" 2>/dev/null && ok "Held in Common Signals arrived signed (ProfileV2)" || no "share not a ProfileV2"
TKID=$(ls "$T/commons/remote/" | head -1 | sed 's/\.json$//')
TPAGE=$(curl -s "http://localhost:8091/federated/$TKID")
printf '%s' "$TPAGE" | grep -q "Withdraw even after training" && printf '%s' "$TPAGE" | grep -q "&lt;b&gt;Never&lt;/b&gt; my voice." && ok "commons shows the Held in Common Signals, escaped" || no "Held in Common Signals not shown on the commons"
curl -s http://localhost:8091/ | grep -qi "tega" && ok "@tega in commons directory" || no "@tega not in directory"
curl -s http://localhost:8091/api/directory | grep -q "tega" && no "tega wrongly local on commons" || ok "tega federated, not local"

# The commons is a federation target, not an identity store: /join and /login are
# gated (HIC_ROLE=commons).
curl -s http://localhost:8091/join | grep -qi "shared commons" && ok "commons /join shows the gate page" || no "commons /join not gated"
curl -s http://localhost:8091/login | grep -qi "shared commons" && ok "commons /login shows the gate page" || no "commons /login not gated"
GSID=$(curl -is -X POST http://localhost:8091/join --data-urlencode "handle=sneak" --data-urlencode "display_name=Sneak" --data-urlencode "passphrase=supersecret9" --data-urlencode "passphrase_again=supersecret9" | tr -d '\r' | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
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
ISID=$(curl -is -X POST http://localhost:8092/join --data-urlencode "handle=tega" --data-urlencode "display_name=Tega Brain" --data-urlencode "passphrase=supersecret3" --data-urlencode "passphrase_again=supersecret3" | tr -d '\r' | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
ICSRF=$(curl -s http://localhost:8092/new -H "Cookie: hic_session=$ISID" | grep -oE 'name="csrf" value="[0-9a-f]+"' | head -1 | grep -oE '[0-9a-f]{8,}')
curl -s -o /dev/null -X POST http://localhost:8092/publish -H "Cookie: hic_session=$ISID" --data-urlencode "handle=tega" --data-urlencode "csrf=$ICSRF" --data-urlencode "name=Tega Brain" --data-urlencode "bio=Definitely the real one." --data-urlencode "visibility=public" --data-urlencode "license=cc-by"
curl -s -X POST http://localhost:8092/share -H "Cookie: hic_session=$ISID" --data-urlencode "handle=tega" --data-urlencode "csrf=$ICSRF" --data-urlencode "commons_url=http://localhost:8091" | grep -q "already used on " && ok "same handle from another node + key refused (409)" || no "impostor @tega was not refused"
grep -rq "Definitely the real one" "$T/commons/remote/" 2>/dev/null && no "impostor profile stored on the commons" || ok "impostor profile not stored"
curl -s -X POST http://localhost:8090/share -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8091" | grep -qi "federated to" && ok "original @tega can still re-share (same key)" || no "original @tega re-share refused"

# Handles are unique per commons, not globally.
curl -s http://localhost:8091/api/handle/tega | grep -q '"available":false' && ok "commons reports @tega taken" || no "api/handle did not report @tega taken"
curl -s http://localhost:8091/api/handle/nobody_yet | grep -q '"available":true' && ok "commons reports a free handle available" || no "api/handle did not report a free handle"
# A node whose default commons already has @tega refuses that handle at sign-up.
mkdir -p "$T/node3"
HIC_DATA="$T/node3" PORT=8093 HIC_ROLE=node HIC_INSTANCE="Third node" HIC_BASE_URL=http://localhost:8093 HIC_SHARE_DEFAULT=http://localhost:8091 "$BIN" >"$T/node3.log" 2>&1 & N3PID=$!
curl -sf --retry 40 --retry-delay 1 --retry-connrefused http://localhost:8093/health >/dev/null || no "third node failed to start"
curl -s -X POST http://localhost:8093/join --data-urlencode "handle=tega" --data-urlencode "display_name=Tega" --data-urlencode "passphrase=supersecret4" --data-urlencode "passphrase_again=supersecret4" | grep -q "already used on " && ok "sign-up refused when default commons has the handle" || no "sign-up allowed a handle taken on the default commons"
[ -e "$T/node3/handles/tega.json" ] && no "refused sign-up still minted @tega" || ok "refused sign-up minted no identity"
# ...but the same handle may live on a different commons (one key per handle per commons).
mkdir -p "$T/commons2"
HIC_DATA="$T/commons2" PORT=8094 HIC_ROLE=commons HIC_INSTANCE="Second commons" HIC_BASE_URL=http://localhost:8094 "$BIN" >"$T/commons2.log" 2>&1 & C2PID=$!
curl -sf --retry 40 --retry-delay 1 --retry-connrefused http://localhost:8094/health >/dev/null || no "second commons failed to start"
FOUND2=$(grep -oE 'hic-founding-[0-9a-f]+' "$T/commons2/founding-invite.json" 2>/dev/null)
curl -s -X POST http://localhost:8092/share -H "Cookie: hic_session=$ISID" --data-urlencode "handle=tega" --data-urlencode "csrf=$ICSRF" --data-urlencode "commons_url=http://localhost:8094" --data-urlencode "invite=$FOUND2" | grep -qi "federated to" && ok "other @tega accepted on a different commons" || no "per-commons uniqueness wrongly global"
# The origin is shown with the card: where it came from + key fingerprint.
# This node says it lives at localhost, which means "its own computer", so
# the card shows the node's name instead, never this computer's address.
HOME_PAGE=$(curl -s http://localhost:8091/)
echo "$HOME_PAGE" | grep -q 'from Tega node' && ok "commons card names a node on its own computer" || no "card missing origin name"
echo "$HOME_PAGE" | grep -qE 'from [^<]*:8090' && no "card shows a localhost origin as an address" || ok "card shows no address for a node on its own computer"
curl -s http://localhost:8091/ | grep -q 'class="dir-key"' && ok "commons card shows key fingerprint" || no "card missing key fingerprint"

# A stand-in "commons" on 8099 that keeps the last thing posted to its inbox.
# Only /inbox: the node later asks it who is waiting (/api/waiting, ADR 0026),
# which must not overwrite the captured share.
python3 - "$T/cap.json" <<'PY' &
import http.server,sys
out=sys.argv[1]
class H(http.server.BaseHTTPRequestHandler):
  def do_POST(s):
    n=int(s.headers.get('Content-Length',0)); b=s.rfile.read(n)
    if s.path!='/inbox':
      s.send_response(404); s.end_headers(); return
    open(out,'wb').write(b)
    s.send_response(200); s.end_headers(); s.wfile.write(b'{"ok":1}')
  def log_message(s,*a): pass
http.server.HTTPServer(('127.0.0.1',8099),H).serve_forever()
PY
CAPPID=$!
# Wait up to 5s for the capture server (Python can take a moment to start on CI).
for _ in $(seq 50); do (exec 3<>/dev/tcp/127.0.0.1/8099) 2>/dev/null && break; sleep 0.1; done
curl -s -o /dev/null -X POST http://localhost:8090/share -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8099"
[ -s "$T/cap.json" ] && ok "captured on-the-wire signed activity" || no "capture failed"

[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8091/inbox -H 'Content-Type: application/json' --data-binary @"$T/cap.json")" = "202" ] && ok "valid activity accepted (202)" || no "valid replay not 202"
sed 's/environmental/TAMPERED/' "$T/cap.json" > "$T/tamper.json"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8091/inbox -H 'Content-Type: application/json' --data-binary @"$T/tamper.json")" = "400" ] && ok "tampered content rejected (400)" || no "tamper not 400"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8091/inbox -H 'Content-Type: application/json' --data-binary 'not json')" = "400" ] && ok "garbage rejected (400)" || no "garbage not 400"

SID2=$(curl -is -X POST http://localhost:8090/join --data-urlencode "handle=mo" --data-urlencode "display_name=Mo Secret" --data-urlencode "passphrase=supersecret2" --data-urlencode "passphrase_again=supersecret2" | tr -d '\r' | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
CSRF2=$(curl -s http://localhost:8090/new -H "Cookie: hic_session=$SID2" | grep -oE 'name="csrf" value="[0-9a-f]+"' | head -1 | grep -oE '[0-9a-f]{8,}')
curl -s -o /dev/null -X POST http://localhost:8090/publish -H "Cookie: hic_session=$SID2" --data-urlencode "handle=mo" --data-urlencode "csrf=$CSRF2" --data-urlencode "name=Mo Secret" --data-urlencode "bio=Private." --data-urlencode "visibility=members" --data-urlencode "license=cc-by"
curl -s -X POST http://localhost:8090/share -H "Cookie: hic_session=$SID2" --data-urlencode "handle=mo" --data-urlencode "csrf=$CSRF2" --data-urlencode "commons_url=http://localhost:8091" | grep -qi "never shared\|stay on your node" && ok "members-only share blocked on node" || no "members-only not blocked"
curl -s http://localhost:8091/ | grep -qi 'Mo Secret\|@mo' && no "members handle leaked to commons" || ok "members handle never reached commons"

# Vouching (ADR 0011): a newcomer without an invite waits; a member vouches for
# them from their own node; then they are listed.
NSID=$(curl -is -X POST http://localhost:8093/join --data-urlencode "handle=newbie" --data-urlencode "display_name=New Person" --data-urlencode "passphrase=supersecret5" --data-urlencode "passphrase_again=supersecret5" | tr -d '\r' | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
NCSRF=$(curl -s http://localhost:8093/new -H "Cookie: hic_session=$NSID" | grep -oE 'name="csrf" value="[0-9a-f]+"' | head -1 | grep -oE '[0-9a-f]{8,}')
curl -s -o /dev/null -X POST http://localhost:8093/publish -H "Cookie: hic_session=$NSID" --data-urlencode "handle=newbie" --data-urlencode "csrf=$NCSRF" --data-urlencode "name=New Person" --data-urlencode "visibility=public" --data-urlencode "license=cc-by"
curl -s -X POST http://localhost:8093/share -H "Cookie: hic_session=$NSID" --data-urlencode "handle=newbie" --data-urlencode "csrf=$NCSRF" --data-urlencode "commons_url=http://localhost:8091" | grep -q "Waiting for a vouch" && ok "newcomer without an invite waits for a vouch" || no "newcomer not put on the pending list"
grep -rq '"newbie"' "$T/commons/remote/" 2>/dev/null && no "newcomer listed without a vouch" || ok "newcomer not listed yet"
NKEY=$(curl -s http://localhost:8091/api/pending | grep -oE '"key":"[0-9a-f]{12}' | head -1 | cut -d'"' -f4)
curl -s -X POST http://localhost:8090/vouch -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8091" --data-urlencode "who=newbie" --data-urlencode "key=$NKEY" | grep -q "Vouched" && ok "member vouched for the newcomer from their node" || no "vouch failed"
grep -rq '"newbie"' "$T/commons/remote/" 2>/dev/null && ok "newcomer listed after the vouch" || no "newcomer still not listed"
curl -s -X POST http://localhost:8090/invite -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8091" | grep -q "Invite code" && ok "member can make an invite code" || no "invite code page failed"

# Vouching with a click (ADR 0026): amy waits; tega's node shows her on the
# People tab (it asks the commons with a request signed by tega's key), and
# one press of Vouch lists her. Nobody else can read the waiting list.
R=$(curl -is -X POST http://localhost:8093/join --data-urlencode "handle=amy" --data-urlencode "display_name=Amy Ng" --data-urlencode "passphrase=supersecret7" --data-urlencode "passphrase_again=supersecret7" | tr -d '\r'); ASID=$(printf '%s\n' "$R" | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
ACSRF=$(curl -s http://localhost:8093/new -H "Cookie: hic_session=$ASID" | grep -oE 'name="csrf" value="[0-9a-f]+"' | head -1 | grep -oE '[0-9a-f]{8,}')
curl -s -o /dev/null -X POST http://localhost:8093/publish -H "Cookie: hic_session=$ASID" --data-urlencode "handle=amy" --data-urlencode "csrf=$ACSRF" --data-urlencode "name=Amy Ng" --data-urlencode "bio=Not for review." --data-urlencode "visibility=public" --data-urlencode "license=cc-by"
curl -s -o /dev/null -X POST http://localhost:8093/share -H "Cookie: hic_session=$ASID" --data-urlencode "handle=amy" --data-urlencode "csrf=$ACSRF" --data-urlencode "commons_url=http://localhost:8091"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8091/api/waiting -H 'Content-Type: application/json' --data-binary '{}')" = "400" ] && ok "an unsigned waiting-list request is refused" || no "unsigned waiting-list request not refused"
PEOPLE=$(curl -s "http://localhost:8090/@tega?tab=people" -H "Cookie: hic_session=$SID")
printf '%s' "$PEOPLE" | grep -q "Vouch for @amy" && printf '%s' "$PEOPLE" | grep -q "Amy Ng" && ok "member's People tab shows amy waiting, with a Vouch button" || no "amy not on the People tab"
printf '%s' "$PEOPLE" | grep -q "Not for review" && no "the People tab shows a waiting person's profile" || ok "the People tab shows no profile content"
AKEY=$(printf '%s' "$PEOPLE" | grep -oE 'name="key" value="[0-9a-f]{64}"' | head -1 | grep -oE '[0-9a-f]{64}')
curl -s -X POST http://localhost:8090/vouch -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "commons_url=http://localhost:8091" --data-urlencode "who=amy" --data-urlencode "key=$AKEY" | grep -q "is now listed" && no "Vouch worked without the CSRF token" || ok "Vouch without the CSRF token is refused"
curl -s -X POST http://localhost:8090/vouch -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8091" --data-urlencode "who=amy" --data-urlencode "key=$AKEY" | grep -q "@amy is now listed on Test Collective" && ok "one click vouched for amy" || no "one-click vouch failed"
grep -rq '"amy"' "$T/commons/remote/" 2>/dev/null && ok "amy listed after the one-click vouch" || no "amy still not listed"

# The steward approves someone waiting (ADR 0017).
R=$(curl -is -X POST http://localhost:8093/join --data-urlencode "handle=walker" --data-urlencode "display_name=Walker" --data-urlencode "passphrase=supersecret6" --data-urlencode "passphrase_again=supersecret6" | tr -d '\r'); WSID=$(printf '%s\n' "$R" | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
WCSRF=$(curl -s http://localhost:8093/new -H "Cookie: hic_session=$WSID" | grep -oE 'name="csrf" value="[0-9a-f]+"' | head -1 | grep -oE '[0-9a-f]{8,}')
curl -s -o /dev/null -X POST http://localhost:8093/publish -H "Cookie: hic_session=$WSID" --data-urlencode "handle=walker" --data-urlencode "csrf=$WCSRF" --data-urlencode "name=Walker" --data-urlencode "visibility=public" --data-urlencode "license=cc-by"
curl -s -o /dev/null -X POST http://localhost:8093/share -H "Cookie: hic_session=$WSID" --data-urlencode "handle=walker" --data-urlencode "csrf=$WCSRF" --data-urlencode "commons_url=http://localhost:8091"
WKEY=$(curl -s http://localhost:8091/api/pending | grep -oE '"handle":"walker","key":"[0-9a-f]{64}' | grep -oE '[0-9a-f]{64}')
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8091/steward/approve -H "Cookie: hic_session=$STEWARD" --data-urlencode "csrf=$SCSRF" --data-urlencode "key=$WKEY")" = "303" ] && grep -rq '"walker"' "$T/commons/remote/" && ok "steward approved someone waiting with one click" || no "steward approval failed"
curl -s http://localhost:8093/@walker -H "Cookie: hic_session=$WSID" | grep -q "Waiting to be listed" && ok "node shows it is waiting, with Check now" || no "no waiting panel"
curl -s -o /dev/null -X POST http://localhost:8093/waiting/check -H "Cookie: hic_session=$WSID" --data-urlencode "csrf=$WCSRF"
curl -s http://localhost:8093/@walker -H "Cookie: hic_session=$WSID" | grep -q "@walker is now listed on " && ok "Check now tells the node it was approved" || no "no 'now listed' notice"

# Stories: tega writes one on their node, shares it to the commons they're
# listed on, then withdraws it; the commons lists it, then doesn't.
SLOC=$(curl -s -D - -o /dev/null -X POST http://localhost:8090/stories/new -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "title=Night bus on Lime Street" --data-urlencode "text=The 38 sighs at the stop. A dog answers." --data-urlencode "visibility=public" --data-urlencode "license=cc-by-nc" --data-urlencode "signals=credit" | grep -i '^location:' | tr -d '\r' | awk '{print $2}')
STORY_ID=$(echo "$SLOC" | cut -d/ -f3)   # /story/<id>/rights: the rights step comes next
[ ${#STORY_ID} -eq 16 ] && ok "node saved a signed story" || no "story not saved ($SLOC)"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8090/story/share -H "Cookie: hic_session=$SID" --data-urlencode "id=$STORY_ID" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8091")" = "303" ] && ok "node shared the story to the commons" || no "story share failed"
curl -s http://localhost:8091/stories | grep -q "Night bus on Lime Street" && ok "commons lists the story" || no "story not listed on the commons"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8090/story/withdraw -H "Cookie: hic_session=$SID" --data-urlencode "id=$STORY_ID" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8091")" = "303" ] && ok "node withdrew the story" || no "story withdrawal failed"
curl -s http://localhost:8091/stories | grep -q "Night bus on Lime Street" && no "withdrawn story still listed" || ok "withdrawn story gone from the commons"
# Edit it and re-share: the commons replaces its copy with the new version.
curl -s -o /dev/null -X POST "http://localhost:8090/story/$STORY_ID/edit" -H "Cookie: hic_session=$SID" --data-urlencode "csrf=$CSRF" --data-urlencode "title=Night bus on Lime St" --data-urlencode "text=The 38 sighs. A dog answers. Later."
sharestory(){ curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8090/story/share -H "Cookie: hic_session=$SID" --data-urlencode "id=$STORY_ID" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8091"; }
[ "$(sharestory)" = "303" ] && curl -s http://localhost:8091/stories | grep -q "Night bus on Lime St<" && ok "a withdrawn, then edited, story can be shared again (new version)" || no "re-sharing an edited story failed"
# A second story, shared from its rights step ("Share to the room").
S2=$(curl -s -D - -o /dev/null -X POST http://localhost:8090/stories/new -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "title=Scaffold rain" --data-urlencode "text=The scaffold keeps playing." | grep -i '^location:' | tr -d '\r' | awk '{print $2}' | cut -d/ -f3)
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://localhost:8090/story/$S2/rights" -H "Cookie: hic_session=$SID" --data-urlencode "csrf=$CSRF" --data-urlencode "visibility=commons" --data-urlencode "license=cc-by" --data-urlencode "signals=credit" --data-urlencode "withdraw_after_training=yes" --data-urlencode "action=share" --data-urlencode "commons_url=http://localhost:8091")" = "303" ] && curl -s http://localhost:8091/stories | grep -q "Scaffold rain" && ok "a new story shared from its rights step reaches the commons" || no "sharing from the rights step failed"
curl -s http://localhost:8091/stories | grep -q ">Withdraw even after training<" && ok "the story's Held in Common Signals show on its card (StoryV2)" || no "story card lacks its Held in Common Signals"

# Key rotation (decision record 0010): tega replaces their key; every commons
# moves @tega to the new key, and the old key stops working there.
OLDKEY=$(curl -s http://localhost:8090/api/@tega | grep -oE '"ed25519_public":"[0-9a-f]+"')
cp "$T/cap.json" "$T/old-key-share.json"   # a share signed by the key about to be replaced
curl -s -X POST http://localhost:8090/rotate -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" | grep -q "Key replaced" && ok "node replaced @tega's key on every commons" || no "key replacement failed"
NEWKEY=$(curl -s http://localhost:8090/api/@tega | grep -oE '"ed25519_public":"[0-9a-f]+"')
[ -n "$NEWKEY" ] && [ "$NEWKEY" != "$OLDKEY" ] && ok "node now signs with the new key" || no "node key unchanged"
grep -q "${NEWKEY#*:\"}" "$T/commons/identities/tega.json" && ok "commons records @tega under the new key" || no "commons didn't move @tega"
curl -s http://localhost:8091/stories | grep -q "Night bus on Lime St<" && no "a story signed by the replaced key is still listed" || ok "stories signed by the replaced key are taken down"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8091/inbox -H 'Content-Type: application/json' --data-binary @"$T/old-key-share.json")" = "409" ] && ok "a share signed by the old key is refused (409)" || no "old key still accepted"
# Recovery: the recovery code takes the handle to fresh keys and issues a new code.
REC=$(curl -s -X POST http://localhost:8090/recover -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "code=$RCODE")
printf '%s' "$REC" | grep -q "Your new recovery code" && ok "recovery code moved @tega to fresh keys" || no "recovery failed"
curl -s http://localhost:8091/ | grep -q "@tega" && ok "@tega still listed on the commons after recovery" || no "@tega lost after recovery"
[ "$(sharestory)" = "303" ] && curl -s http://localhost:8091/stories | grep -q "Night bus on Lime St<" && ok "after recovery the story is re-signed and shared again" || no "story not re-shared after recovery"

# Withdrawal: @tega asks the commons to remove its copy. The commons drops it,
# keeps @tega reserved for tega's key, and refuses a replay of the old share.
curl -s -X POST http://localhost:8090/withdraw -H "Cookie: hic_session=$SID" --data-urlencode "handle=tega" --data-urlencode "csrf=$CSRF" --data-urlencode "commons_url=http://localhost:8091" | grep -q "Withdrawn" && ok "node withdrew @tega from the commons" || no "withdrawal failed"
grep -rq '"tega"' "$T/commons/remote/" 2>/dev/null && no "withdrawn copy still stored" || ok "withdrawn copy removed from the commons"
curl -s http://localhost:8091/stories | grep -q "Night bus on Lime St<" && no "stories stayed after the profile was withdrawn" || ok "withdrawing the profile took the story with it"
curl -s "http://localhost:8090/story/$STORY_ID" -H "Cookie: hic_session=$SID" | grep -q ">Shared to<" && no "node still marks the story as shared" || ok "node no longer marks the story as shared"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8091/inbox -H 'Content-Type: application/json' --data-binary @"$T/cap.json")" = "409" ] && ok "withdrawn version can't be replayed (409)" || no "withdrawn version was accepted again"
curl -s http://localhost:8091/api/handle/tega | grep -q '"available":false' && ok "@tega stays reserved after withdrawal" || no "@tega freed by withdrawal"

# "Only while I'm here" (decision record 0021): @walker (third node, listed on
# the commons) shares a story as a card. The commons keeps no text; readers get
# the words from walker's node over the node's own long poll; when the node
# stops, readers see "Not here right now"; when it's back, the words are too;
# a withdrawal removes it at once.
HTEXT=$'The shelter hums at the stop.\nA man whistles along in B flat.'
HLOC=$(curl -s -D - -o /dev/null -X POST http://localhost:8093/stories/new -H "Cookie: hic_session=$WSID" --data-urlencode "handle=walker" --data-urlencode "csrf=$WCSRF" --data-urlencode "title=Bus stop hum" --data-urlencode "text=$HTEXT" | grep -i '^location:' | tr -d '\r' | awk '{print $2}')
HID=$(echo "$HLOC" | cut -d/ -f3)
curl -s -o /dev/null -X POST "http://localhost:8093/story/$HID/rights" -H "Cookie: hic_session=$WSID" --data-urlencode "csrf=$WCSRF" --data-urlencode "visibility=commons" --data-urlencode "license=cc-by" --data-urlencode "travel=here" --data-urlencode "action=keep"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8093/story/share -H "Cookie: hic_session=$WSID" --data-urlencode "id=$HID" --data-urlencode "csrf=$WCSRF" --data-urlencode "commons_url=http://localhost:8091")" = "303" ] && ok "node shared a story as Only while I'm here" || no "sharing Only while I'm here failed"
HLIST=$(curl -s http://localhost:8091/stories)
printf '%s' "$HLIST" | grep -q "Only while @walker is here" && ok "commons lists the card" || no "card not listed"
grep -rq "B flat" "$T/commons" && no "the commons stored the words" || ok "the commons keeps only the card, no words"
HPATH=$(printf '%s' "$HLIST" | grep -oE "/stories/[0-9a-f]+/$HID" | head -1)
# read: wait (up to ~15 s) for the node's connection, then for the words.
read_here(){ for _ in $(seq 15); do curl -s "http://localhost:8091$HPATH" | grep -q "$1" && return 0; sleep 1; done; return 1; }
read_here "B flat" && ok "a reader got the words from the author's node" || no "reader didn't get the words"
# The words match the hash the author signed (checked here independently).
curl -s "http://localhost:8091$HPATH/here.json" > "$T/here.json"
python3 - "$T/here.json" <<'PY' && ok "the words match the card's signed SHA-256" || no "words don't match the signed hash"
import hashlib, json, sys
got = json.load(open(sys.argv[1]))
card = got["card"]["envelope"]["body"]
ok = "text" in got and hashlib.sha256(got["text"].encode("utf-8")).hexdigest() == card["text_sha256"]
if not ok:
    print("    here.json:", {k: v for k, v in got.items() if k != "card"})
sys.exit(0 if ok else 1)
PY
curl -s -D - -o /dev/null "http://localhost:8091$HPATH" | grep -qi '^cache-control: no-store' && ok "the commons tells browsers not to keep it" || no "no Cache-Control: no-store"
grep -rq "B flat" "$T/commons" && no "the words were written to the commons' disk" || ok "after reading, still no words on the commons' disk"
# The author's node stops (laptop closed).
kill $N3PID 2>/dev/null; wait $N3PID 2>/dev/null
curl -s "http://localhost:8091$HPATH" | grep -q "Not here right now: @walker shares this only while they're online." && ok "with the node gone, readers see Not here right now" || no "no Not here right now"
# ...and comes back: the words are readable again, with no action needed.
HIC_DATA="$T/node3" PORT=8093 HIC_ROLE=node HIC_INSTANCE="Third node" HIC_BASE_URL=http://localhost:8093 HIC_SHARE_DEFAULT=http://localhost:8091 "$BIN" >>"$T/node3.log" 2>&1 & N3PID=$!
curl -sf --retry 40 --retry-delay 1 --retry-connrefused http://localhost:8093/health >/dev/null || no "third node failed to restart"
read_here "B flat" && ok "after the node restarts, readers get the words again" || no "words not back after restart"
# Withdraw (sessions don't survive a restart, so sign in again).
WSID=$(curl -is -X POST http://localhost:8093/login --data-urlencode "handle=walker" --data-urlencode "passphrase=supersecret6" | tr -d '\r' | awk -F'hic_session=' '/[Ss]et-[Cc]ookie/{split($2,a,";"); print a[1]}')
WCSRF=$(curl -s http://localhost:8093/new -H "Cookie: hic_session=$WSID" | grep -oE 'name="csrf" value="[0-9a-f]+"' | head -1 | grep -oE '[0-9a-f]{8,}')
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://localhost:8093/story/withdraw -H "Cookie: hic_session=$WSID" --data-urlencode "id=$HID" --data-urlencode "csrf=$WCSRF" --data-urlencode "commons_url=http://localhost:8091")" = "303" ] && ok "node withdrew the Only while I'm here story" || no "withdrawing the card failed"
[ "$(curl -s -o /dev/null -w '%{http_code}' "http://localhost:8091$HPATH")" = "404" ] && ok "a withdrawn card can't be read, at once" || no "withdrawn card still readable"
curl -s http://localhost:8091/stories | grep -q "Bus stop hum" && no "withdrawn card still listed" || ok "withdrawn card gone from the list"

# Open to the room (ADR 0027): a node listening on the room's network with the
# door in front of it. Fetching this computer's own LAN address makes the
# request arrive from that address, not 127.0.0.1, so it counts as someone in
# the room; no second device is needed.
LAN=$(hostname -I 2>/dev/null | awk '{print $1}')
if [ -z "$LAN" ] && command -v ipconfig >/dev/null; then LAN=$(ipconfig getifaddr en0 2>/dev/null); fi
if [ -z "$LAN" ]; then
  echo "  SKIP  open to the room: this computer has no network address"
else
  mkdir -p "$T/room"
  HIC_DATA="$T/room" PORT=8095 HIC_ROLE=node HIC_BIND="$LAN" HIC_ROOM=1 HIC_ROOM_ADDRESS="http://$LAN:8095" HIC_INSTANCE="Room node" HIC_BASE_URL="http://$LAN:8095" "$BIN" >"$T/room.log" 2>&1 & RPID=$!
  curl -sf --retry 40 --retry-delay 1 --retry-connrefused http://127.0.0.1:8095/health >/dev/null || no "room node failed to start"
  curl -s -X POST http://127.0.0.1:8095/join --data-urlencode "handle=ana" --data-urlencode "display_name=Ana" --data-urlencode "passphrase=supersecret7" --data-urlencode "passphrase_again=supersecret7" -o /dev/null
  code(){ curl -s -o /dev/null -w '%{http_code}' "$@"; }
  [ "$(code "http://$LAN:8095/@ana")" = "200" ] && ok "the room reads a profile on an open node" || no "the room can't read a profile"
  [ "$(code "http://$LAN:8095/login")" = "403" ] && ok "the room can't open the sign-in page" || no "the room reached the sign-in page"
  [ "$(code -X POST "http://$LAN:8095/login" --data-urlencode "handle=ana" --data-urlencode "passphrase=guess")" = "403" ] && ok "the room can't guess passphrases" || no "the room could post a sign-in"
  [ "$(code http://127.0.0.1:8095/login)" = "200" ] && ok "the owner still reaches the sign-in page" || no "the owner was refused"
  [ "$(code -H "X-Forwarded-For: 203.0.113.9" http://127.0.0.1:8095/login)" = "403" ] && ok "a request through a proxy is a room visitor" || no "a proxied request counted as the owner"
  # Ana hasn't published, so the room couldn't open her profile: the code leads to the front page.
  curl -s http://127.0.0.1:8095/room | grep -q "<code>http://$LAN:8095/</code>" && ok "the owner's room page shows the address the room uses" || no "room page missing the room address"
  [ "$(code "http://$LAN:8095/room")" = "403" ] && ok "the room can't see the room page" || no "the room saw the room page"
  [ "$(code "http://$LAN:8095/room/check")" = "200" ] && ok "the room can check it reached this computer" || no "room check failed"
  kill $RPID 2>/dev/null; wait $RPID 2>/dev/null
fi

echo ""; echo "RESULT: $PASS passed, $FAIL failed"
exit $FAIL
