// The desktop app's own screens: the first-launch chooser (node or commons)
// and the loading screen shown while the server starts.
//
// Talks to the Rust side through Tauri commands (src-tauri/src/commands.rs):
//   setup_state  -> { role, name, lan_url }   chosen and named yet?
//   choose_role  (role, name)           save the choice and name, start the server
//   set_name     (name)                 name (or rename) this computer's node
//   node_up      -> bool                is *our* server answering yet?
//   restart_node                        start (or restart) the server
// When opened in an ordinary browser (no Tauri), it just waits for the server.

// The local server's address. 127.0.0.1 rather than "localhost": a node
// listens only on IPv4 loopback, and "localhost" may resolve to IPv6 first.
const url = 'http://127.0.0.1:8080';
const invoke = window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke;
const $ = id => document.getElementById(id);

// ---- loading screen -------------------------------------------------
const MSGS = {
  node: ['Warming up the commons…', 'Minting your keys…', 'Opening your node…'],
  commons: ['Warming up the commons…', 'Opening the directory…', 'Getting ready for visitors…']
};
let msgs = MSGS.node, n = 0, done = false;

// Is the server listening? Prefer the host-side command (a fetch to
// http://localhost from this secure origin is blocked as mixed content). Fall
// back to fetch when running outside the app (e.g. opened in a browser).
async function isUp(){
  if (invoke){ try { return await invoke('node_up'); } catch(e){ return false; } }
  try { const r = await fetch(url + '/health', {cache:'no-store'}); return r.ok; } catch(e){ return false; }
}

async function restart(){
  $('err').textContent = '';
  if (!invoke){ location.href = url; return; }
  const b = $('start'); b.disabled = true;
  const was = b.textContent; b.textContent = 'Starting…';
  $('s').textContent = 'Starting…';
  try { await invoke('restart_node'); }
  catch(e){ $('err').textContent = ('' + e) || 'Could not start.'; }
  setTimeout(() => { b.disabled = false; b.textContent = was; }, 1200);
}
$('start').addEventListener('click', restart);

async function poll(){
  if (done) return;
  n++;
  if (n <= msgs.length * 3) $('s').textContent = msgs[Math.min(((n/3)|0), msgs.length-1)];
  if (await isUp()){ done = true; location.href = url; return; }
  // ~12s of patience, then reveal the manual Start button + fallback link.
  if (n > 20){
    $('s').textContent = 'Taking longer than usual.';
    $('actions').classList.add('show');
  }
  setTimeout(poll, 600);
}

function showLoading(role){
  const what = role === 'commons' ? 'commons' : 'node';
  msgs = MSGS[what];
  $('loading-title').firstChild.textContent = 'Starting your ' + what;
  $('start').textContent = 'Start the ' + what;
  $('chooser').hidden = true;
  $('loading').hidden = false;
  poll();
}

// ---- first launch: choose node or commons, then name it ---------------
// `naming` is { role, firstLaunch }: firstLaunch means the role isn't saved yet.
let naming = null;

function choose(role){
  $('choose-err').textContent = '';
  showNaming(role, true);
}

function showNaming(role, firstLaunch){
  naming = { role, firstLaunch };
  $('naming-role').textContent = role === 'commons' ? 'commons' : 'node';
  $('naming-eyebrow').textContent = firstLaunch ? 'Held in Common · first launch' : 'Held in Common · one new step';
  $('naming-why').hidden = firstLaunch;
  $('name-back').hidden = !firstLaunch;
  $('chooser').hidden = true;
  $('naming').hidden = false;
  $('node-name').focus();
}

$('name-back').addEventListener('click', () => {
  $('naming').hidden = true;
  $('chooser').hidden = false;
});

$('name-form').addEventListener('submit', async e => {
  e.preventDefault();
  const name = $('node-name').value.trim();
  if (!name){ $('name-err').textContent = 'Give it a name first.'; return; }
  $('name-err').textContent = '';
  $('name-go').disabled = true;
  try {
    if (naming.firstLaunch) await invoke('choose_role', { role: naming.role, name });
    else await invoke('set_name', { name });
    $('naming').hidden = true;
    showLoading(naming.role);
  } catch(err){
    $('name-err').textContent = ('' + err) || 'Could not start.';
    $('name-go').disabled = false;
  }
});

function showChooser(lanUrl){
  if (lanUrl){
    const d = $('room-detail');
    d.textContent = 'People on the same Wi-Fi or network share to ';
    const c = document.createElement('code'); c.textContent = lanUrl; d.appendChild(c);
  } else {
    $('room-detail').textContent = "We couldn't find this computer's network address. Connect to Wi-Fi, then reopen the app.";
    $('start-commons').disabled = true;
  }
  $('pick-node').addEventListener('click', () => choose('node'));
  $('pick-commons').addEventListener('click', () => {
    $('pick-commons').setAttribute('aria-pressed', 'true');
    $('reach').hidden = false;
    $('reach-room').focus();
  });
  $('back').addEventListener('click', () => {
    $('pick-commons').setAttribute('aria-pressed', 'false');
    $('reach').hidden = true;
    $('pick-commons').focus();
  });
  $('start-commons').addEventListener('click', () => choose('commons'));
  $('chooser').hidden = false;
}

async function init(){
  // Outside the app (opened in a browser) there is nothing to choose: just wait for the server.
  if (!invoke){ showLoading('node'); return; }
  let st = null;
  try { st = await invoke('setup_state'); } catch(e){ st = { role: 'node', name: 'node' }; }
  if (st.role && st.name) showLoading(st.role);
  else if (st.role) showNaming(st.role, false);   // installed before names existed
  else showChooser(st.lan_url);
}
init();
