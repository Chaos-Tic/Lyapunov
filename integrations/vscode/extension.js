// A local presence beacon for one VS Code window. Only a canonical folder
// path is written; no titles, files, tabs, or network requests are involved.
const vscode = require('vscode');
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

let timer;
let marker;

function runtimeDirectory() {
  if (process.platform !== 'linux') return undefined;
  const root = process.env.XDG_RUNTIME_DIR;
  if (!root || !path.isAbsolute(root)) return undefined;
  const dir = path.join(root, 'lyapunov-vscode');
  fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
  if (fs.lstatSync(dir).isSymbolicLink()) return undefined;
  fs.chmodSync(dir, 0o700);
  return dir;
}

function currentFolder() {
  const folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length !== 1 || folders[0].uri.scheme !== 'file') return undefined;
  try { return fs.realpathSync(folders[0].uri.fsPath); } catch { return undefined; }
}

function refresh() {
  if (!marker) return;
  const folder = currentFolder();
  if (!folder) {
    try { fs.unlinkSync(marker); } catch {}
    return;
  }
  const tmp = marker + '.tmp';
  try {
    fs.writeFileSync(tmp, JSON.stringify({ folder, pid: process.pid }), { mode: 0o600 });
    fs.renameSync(tmp, marker);
  } catch {
    try { fs.unlinkSync(tmp); } catch {}
  }
}

function activate(context) {
  const dir = runtimeDirectory();
  if (!dir) return;
  marker = path.join(dir, crypto.randomUUID() + '.json');
  refresh();
  timer = setInterval(refresh, 2000);
  context.subscriptions.push(vscode.workspace.onDidChangeWorkspaceFolders(refresh));
}

function deactivate() {
  if (timer) clearInterval(timer);
  if (marker) { try { fs.unlinkSync(marker); } catch {} }
}

module.exports = { activate, deactivate };
