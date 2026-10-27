import init, {
  init_shell, handle_input, get_prompt, get_cwd,
  upload_file, download_file, list_extra_keys
} from '../pkg/mobile_linux_shell.js';

const term = new Terminal({
  cursorBlink: true,
  fontSize: 14,
  fontFamily: '"DejaVu Sans Mono", "Cascadia Code", monospace',
  theme: {
    background: '#000000',
    foreground: '#cccccc',
    cursor: '#00ff00',
    selectionBackground: '#444444'
  },
  allowProposedApi: true,
  scrollback: 5000
});

const fitAddon = new FitAddon.FitAddon();
term.loadAddon(fitAddon);
term.open(document.getElementById('terminal'));
fitAddon.fit();

let currentLine = '';
let ctrlPressed = false;
let altPressed = false;

async function boot() {
  await init();
  await init_shell();
  term.writeln('\x1b[1;32mMobile Linux Shell\x1b[0m – WASM core ready');
  term.writeln('Type \x1b[1mhelp\x1b[0m for built-in commands.\r\n');
  prompt();
  buildExtraKeys();
  setupViewport();
  setupFileBridge();
}

function prompt() {
  const p = get_prompt();
  term.write(p);
  currentLine = '';
}

term.onData(data => {
  if (data === '\r') {               // Enter
    term.write('\r\n');
    runCommand(currentLine);
  } else if (data === '\x7f') {      // Backspace
    if (currentLine.length > 0) {
      currentLine = currentLine.slice(0, -1);
      term.write('\b \b');
    }
  } else if (data === '\x03') {      // Ctrl+C
    term.write('^C\r\n');
    prompt();
  } else if (data >= String.fromCharCode(0x20)) {
    currentLine += data;
    term.write(data);
  }
});

async function runCommand(line) {
  try {
    const out = await handle_input(line);
    if (out) term.write(out.replace(/\n/g, '\r\n'));
  } catch (e) {
    term.writeln(`\x1b[31mError: ${e}\x1b[0m`);
  }
  prompt();
}

// ---------- Termux-style Extra Keys ----------
function buildExtraKeys() {
  const container = document.getElementById('extra-keys');
  const keys = [
    // row 1
    ['ESC', 'TAB', 'CTRL', 'ALT', '↑', '↓', '←', '→'],
    // row 2
    ['/', '-', 'HOME', 'END', 'PGUP', 'PGDN', 'Ctrl+C', 'Paste']
  ];

  keys.forEach(row => {
    const r = document.createElement('div');
    r.className = 'ek-row';
    row.forEach(label => {
      const k = document.createElement('div');
      k.className = 'ek-key' + (['CTRL','ALT','ESC','Ctrl+C'].includes(label) ? ' special' : '');
      k.textContent = label;
      k.addEventListener('touchstart', e => { e.preventDefault(); handleExtraKey(label, k); }, {passive:false});
      k.addEventListener('mousedown', e => { e.preventDefault(); handleExtraKey(label, k); });
      r.appendChild(k);
    });
    container.appendChild(r);
  });
}

function handleExtraKey(label, el) {
  el.classList.add('active');
  setTimeout(() => el.classList.remove('active'), 120);

  switch (label) {
    case 'ESC':   term.focus(); term.textarea.dispatchEvent(new KeyboardEvent('keydown', {key:'Escape', bubbles:true})); break;
    case 'TAB':   currentLine += '\t'; term.write('    '); break;
    case 'CTRL':  ctrlPressed = !ctrlPressed; el.classList.toggle('active', ctrlPressed); break;
    case 'ALT':   altPressed  = !altPressed;  el.classList.toggle('active', altPressed);  break;
    case '↑':     /* history would go here */ break;
    case '↓':     break;
    case '←':     if (currentLine.length) { currentLine = currentLine.slice(0,-1); term.write('\b \b'); } break;
    case '→':     break;
    case '/':     currentLine += '/'; term.write('/'); break;
    case '-':     currentLine += '-'; term.write('-'); break;
    case 'HOME':  break;
    case 'END':   break;
    case 'PGUP':  term.scrollPages(-1); break;
    case 'PGDN':  term.scrollPages(1);  break;
    case 'Ctrl+C':
      term.write('^C\r\n');
      prompt();
      break;
    case 'Paste':
      navigator.clipboard.readText().then(t => {
        currentLine += t;
        term.write(t);
      }).catch(() => {});
      break;
  }
}

// ---------- visualViewport (Android keyboard) ----------
function setupViewport() {
  const vv = window.visualViewport;
  if (!vv) return;
  const update = () => {
    document.documentElement.style.height = vv.height + 'px';
    fitAddon.fit();
  };
  vv.addEventListener('resize', update);
  vv.addEventListener('scroll', update);
  update();
}

// ---------- File upload / download bridge ----------
function setupFileBridge() {
  const uploadInput = document.getElementById('file-upload');
  // Expose a simple command-line helper via a global for now
  window.__upload = () => uploadInput.click();
  uploadInput.addEventListener('change', async () => {
    for (const file of uploadInput.files) {
      const buf = new Uint8Array(await file.arrayBuffer());
      const path = `/root/${file.name}`;
      await upload_file(path, buf);
      term.writeln(`\r\nuploaded → ${path}`);
    }
    prompt();
  });

  window.__download = async (path) => {
    try {
      const data = await download_file(path);
      const blob = new Blob([data]);
      const url = URL.createObjectURL(blob);
      const a = document.getElementById('file-download');
      a.href = url;
      a.download = path.split('/').pop();
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      term.writeln(`download error: ${e}`);
    }
  };
}

// Resize observer for orientation changes
window.addEventListener('resize', () => fitAddon.fit());

boot().catch(e => {
  document.body.innerHTML = `<pre style="color:red;padding:1rem">Failed to start WASM: ${e}</pre>`;
});
