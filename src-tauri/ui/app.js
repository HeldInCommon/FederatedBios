// The desktop app's own screens: the first-launch chooser (node or commons),
// naming, the loading screen shown while a server starts, and, in the small
// setup window the Go menu opens, adding, renaming or removing a server.
//
// The setup window says what to do in its address (?do=…&role=…):
//   do=add&role=node|commons      run the other role too: name it, start it
//   do=rename&role=node|commons|practice   rename it
//   do=remove-practice            confirm removing the local commons
//   do=steward-link               show the commons' steward link (ADR 0024)
//
// Talks to the Rust side through Tauri commands (src-tauri/src/commands.rs):
//   setup_state  -> { role, name, node_name, commons_name, practice_name, practice, lan_url }
//   choose_role  (role, name)           first launch: save the choice and name, start it
//   add_role     (role, name)           add the other role and start it
//   set_name     (role, name)           name (or rename) the node or commons
//   remove_practice_now                 delete the local commons
//   steward_links -> [string]           the commons' steward link, per address
//   close_setup                         close the setup window
//   node_up      (which) -> bool        is *our* server of that kind answering yet?
//   port_problem (which) -> string|null why it hasn't started (another program has the port)
//   server_url   (which) -> string      where the window goes to show it
//   restart_node (which)                start (or restart) it
// `which` is "node", "commons" or "practice"; left out, the first-launch role.
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
// Which server the loading screen waits for (undefined: the first-launch
// role), and what to do once it answers: go there, or close this setup window.
let waitFor, whenUp = 'go';

// Is the server listening? Prefer the host-side command (a fetch to
// http://localhost from this secure origin is blocked as mixed content). Fall
// back to fetch when running outside the app (e.g. opened in a browser).
async function isUp(){
  if (invoke){ try { return await invoke('node_up', { which: waitFor }); } catch(e){ return false; } }
  try { const r = await fetch(url + '/health', {cache:'no-store'}); return r.ok; } catch(e){ return false; }
}

async function restart(){
  $('err').textContent = '';
  if (!invoke){ location.href = url; return; }
  const b = $('start'); b.disabled = true;
  const was = b.textContent; b.textContent = 'Starting…';
  $('s').textContent = 'Starting…';
  try { await invoke('restart_node', { which: waitFor }); }
  catch(e){ $('err').textContent = ('' + e) || 'Could not start.'; }
  setTimeout(() => { b.disabled = false; b.textContent = was; }, 1200);
}
$('start').addEventListener('click', restart);

async function poll(){
  if (done) return;
  n++;
  if (n <= msgs.length * 3) $('s').textContent = msgs[Math.min(((n/3)|0), msgs.length-1)];
  if (await isUp()){ done = true; await arrive(); return; }
  // ~12s of patience, then reveal the manual Start button, and say so if
  // another program holds the port (ADR 0008: we never open a server we
  // didn't start, so the person needs to know why nothing is happening).
  if (n > 20){
    $('s').textContent = 'Taking longer than usual.';
    $('actions').classList.add('show');
    if (invoke){
      try {
        const problem = await invoke('port_problem', { which: waitFor });
        if (problem) $('err').textContent = problem;
      } catch(e){}
    }
  }
  setTimeout(poll, 600);
}

// The server is up: show it here, or (in the setup window, where the main
// window switches to it by itself) close this window.
async function arrive(){
  if (whenUp === 'close'){ invoke('close_setup'); return; }
  let to = url + '/';
  if (invoke){ try { to = await invoke('server_url', { which: waitFor }); } catch(e){} }
  location.href = to;
}

// `which` is the kind to wait for (undefined: the first-launch role, whose
// role is `role`).
function showLoading(role, which){
  const what = role === 'commons' ? 'commons' : 'node';
  msgs = MSGS[what];
  waitFor = which;
  $('loading-title').firstChild.textContent = 'Starting your ' + what;
  $('start').textContent = 'Start the ' + what;
  $('chooser').hidden = true;
  $('loading').hidden = false;
  poll();
}

// ---- choose node or commons, then name it --------------------------------
// `naming` is { role, mode } where mode is:
//   'first'   first launch: the role isn't saved yet (choose_role saves both)
//   'migrate' installed before names existed: name it once (set_name)
//   'rename'  opened from Go → Rename… (set_name)
//   'add'     opened from Go, or a page's link, to run the other role too (add_role)
let naming = null, lanUrl = null;

const NAMING_TEXT = {
  first:   { eyebrow: 'Held in Common · first launch', note: '', go: 'Start' },
  migrate: { eyebrow: 'Held in Common · one new step', go: 'Start',
             note: "Earlier versions used this computer's account name. From now on they'll use the name you choose here." },
  rename:  { eyebrow: 'Held in Common · rename', go: 'Save name',
             note: 'Commons that already have your profile show the new name the next time you share to them.' },
  add:     { eyebrow: 'Held in Common · add to this computer', go: 'Start', note: '' },
};

const ADD_NOTE = {
  node: 'Your node runs beside your commons, with its own folder and keys. Share your profile from it to your commons, like anyone else would.',
  commons: 'Your commons runs beside your node. Your node, its keys and your sign-in stay as they are.',
};

function choose(role){
  $('choose-err').textContent = '';
  showNaming(role, 'first', '');
}

function showNaming(role, mode, current){
  naming = { role, mode };
  const t = NAMING_TEXT[mode];
  $('naming-role').textContent = { commons: 'commons', practice: 'local commons' }[role] || 'node';
  $('naming-eyebrow').textContent = t.eyebrow;
  const note = mode === 'add' ? ADD_NOTE[role] : t.note;
  $('naming-why').textContent = note;
  $('naming-why').hidden = !note;
  // Adding a room commons: say where people will reach it.
  const reach = $('naming-reach');
  reach.hidden = !(mode === 'add' && role === 'commons');
  if (!reach.hidden){
    reach.textContent = '';
    if (lanUrl){
      reach.append('People on the same Wi-Fi or network will share to ');
      const c = document.createElement('code'); c.textContent = lanUrl; reach.append(c, '.');
    } else {
      reach.textContent = "We couldn't find this computer's network address. Connect to Wi-Fi first, or people won't be able to reach your commons.";
    }
  }
  $('name-go').textContent = t.go;
  $('name-back').hidden = mode === 'migrate';
  $('node-name').value = current || '';
  $('chooser').hidden = true;
  $('naming').hidden = false;
  $('node-name').focus();
  $('node-name').select();
}

$('name-back').addEventListener('click', () => {
  if (naming && (naming.mode === 'rename' || naming.mode === 'add')){ invoke('close_setup'); return; }
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
    if (naming.mode === 'first') await invoke('choose_role', { role: naming.role, name });
    else if (naming.mode === 'add') await invoke('add_role', { role: naming.role, name });
    else await invoke('set_name', { role: naming.mode === 'rename' ? naming.role : undefined, name });
    if (naming.mode === 'rename') return;   // the app closes this window and reopens the server
    $('naming').hidden = true;
    if (naming.mode === 'add'){ whenUp = 'close'; showLoading(naming.role, naming.role); }
    else showLoading(naming.role);
  } catch(err){
    $('name-err').textContent = ('' + err) || 'Could not start.';
    $('name-go').disabled = false;
  }
});

function showRemove(){
  $('remove').hidden = false;
  $('remove-back').addEventListener('click', () => invoke('close_setup'));
  $('remove-go').addEventListener('click', async () => {
    $('remove-go').disabled = true;
    $('remove-err').textContent = '';
    try { await invoke('remove_practice_now'); }
    catch(err){ $('remove-err').textContent = ('' + err) || 'Could not remove it.'; $('remove-go').disabled = false; }
  });
}

// The steward link: one row per address, each with a Copy button. If the
// clipboard can't be reached, the link is selected so the person can copy
// it themselves.
async function showStewardLink(){
  $('steward').hidden = false;
  $('steward-close').addEventListener('click', () => invoke('close_setup'));
  let links = [];
  try { links = await invoke('steward_links'); }
  catch(err){ $('steward-err').textContent = ('' + err) || 'Could not find the steward link.'; return; }
  for (const link of links){
    const li = document.createElement('li');
    const code = document.createElement('code'); code.textContent = link;
    const copy = document.createElement('button');
    copy.className = 'link'; copy.type = 'button'; copy.textContent = 'Copy';
    copy.addEventListener('click', async () => {
      try { await navigator.clipboard.writeText(link); copy.textContent = 'Copied'; }
      catch(e){
        getSelection().selectAllChildren(code);
        copy.textContent = 'Selected: copy with \u2318C or Ctrl+C';
      }
    });
    li.append(code, copy);
    $('steward-links').append(li);
  }
}

function showChooser(){
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
  lanUrl = st.lan_url;
  // The setup window (opened from the Go menu or a page's link).
  const q = new URLSearchParams(location.search);
  const asked = q.get('role');
  const todo = q.get('do'), role = asked === 'commons' || asked === 'practice' ? asked : 'node';
  if (todo === 'add') showNaming(role, 'add', '');
  else if (todo === 'rename') showNaming(role, 'rename', { node: st.node_name, commons: st.commons_name, practice: st.practice_name }[role]);
  else if (todo === 'remove-practice') showRemove();
  else if (todo === 'steward-link') showStewardLink();
  // The main window.
  else if (st.role && st.name) showLoading(st.role);
  else if (st.role) showNaming(st.role, 'migrate', '');             // installed before names existed
  else showChooser();
}
init();
