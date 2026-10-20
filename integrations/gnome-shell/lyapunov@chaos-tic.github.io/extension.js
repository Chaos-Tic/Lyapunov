// Lyapunov for GNOME Shell.
//
// GNOME does not let applications list or move other applications' windows.
// This extension offers two methods on the session bus so the Lyapunov app
// can capture and resume a space: Snapshot() describes screens, workspaces
// and application windows; Place(json) moves windows. Both answer in the
// JSON shapes the KWin backend uses. Window titles are returned for the
// capture preview only. Nothing is stored and nothing leaves the computer.
//
// Only the Lyapunov executable may call it: the caller's process is looked up
// on the bus, and must run a lyapunov binary installed by a package or
// trusted by the user (`lyapunov gnome-extension`). Any other program
// receives an access-denied error.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'io.github.ChaosTic.Lyapunov.Shell';
const OBJECT_PATH = '/io/github/ChaosTic/Lyapunov/Shell';
const INTERFACE = `<node>
  <interface name="${BUS_NAME}">
    <method name="Snapshot"><arg type="s" direction="out" name="json"/></method>
    <method name="Place"><arg type="s" direction="in" name="ops"/><arg type="s" direction="out" name="json"/></method>
    <property name="Version" type="u" access="read"/>
  </interface>
</node>`;

// Lyapunov executables the user trusted with `lyapunov gnome-extension`
// (installs outside the system folders: AppImage, ~/.local).
function trustedPaths() {
    try {
        const file = GLib.build_filenamev([GLib.get_user_config_dir(), 'lyapunov', 'gnome-trusted']);
        const [, bytes] = GLib.file_get_contents(file);
        return new TextDecoder().decode(bytes).split('\n').map(l => l.trim()).filter(l => l.startsWith('/'));
    } catch (e) {
        return [];
    }
}

// Owned by root and writable by nobody else.
function rootOnly(path) {
    const info = Gio.File.new_for_path(path).query_info('unix::uid,unix::mode', Gio.FileQueryInfoFlags.NONE, null);
    return info.get_attribute_uint32('unix::uid') === 0 && (info.get_attribute_uint32('unix::mode') & 0o022) === 0;
}

// Installed by a package: the executable and its folder belong to root.
function systemInstalled(path) {
    return rootOnly(path) && rootOnly(GLib.path_get_dirname(path));
}

// A process started with libraries injected is not the executable it names.
function injected(pid) {
    try {
        const [, bytes] = GLib.file_get_contents(`/proc/${pid}/environ`);
        return new TextDecoder().decode(bytes).split('\0')
            .some(v => v.startsWith('LD_PRELOAD=') || v.startsWith('LD_LIBRARY_PATH=') || v.startsWith('LD_AUDIT='));
    } catch (e) {
        return true;
    }
}

// The executable behind a bus caller must be Lyapunov's core, installed by a
// package or trusted by the user. A deleted or renamed binary is refused.
function callerIsLyapunov(invocation) {
    try {
        const reply = invocation.get_connection().call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
            'org.freedesktop.DBus', 'GetConnectionUnixProcessID', new GLib.Variant('(s)', [invocation.get_sender()]),
            new GLib.VariantType('(u)'), Gio.DBusCallFlags.NONE, 1000, null);
        const [pid] = reply.deepUnpack();
        const exe = GLib.file_read_link(`/proc/${pid}/exe`);
        if (exe.endsWith(' (deleted)') || GLib.path_get_basename(exe) !== 'lyapunov' || injected(pid))
            return false;
        return systemInstalled(exe) || trustedPaths().includes(exe);
    } catch (e) {
        return false;
    }
}

function guarded(invocation, answer) {
    if (!callerIsLyapunov(invocation)) {
        invocation.return_dbus_error('org.freedesktop.DBus.Error.AccessDenied',
            'Only Lyapunov may list or arrange windows through this extension');
        return;
    }
    try {
        invocation.return_value(new GLib.Variant('(s)', [answer()]));
    } catch (e) {
        invocation.return_dbus_error('org.freedesktop.DBus.Error.Failed', String(e));
    }
}

function rect(r) {
    return {x: r.x, y: r.y, width: r.width, height: r.height};
}

// Connector names (DP-1) from Mutter's logical monitors when it exposes them,
// positions otherwise.
function monitorName(index) {
    try {
        const manager = global.backend.get_monitor_manager();
        for (const logical of manager.get_logical_monitors?.() ?? []) {
            if (logical.get_number() !== index)
                continue;
            const monitor = logical.get_monitors()[0];
            const connector = monitor?.get_connector?.();
            if (connector)
                return connector;
        }
    } catch (e) {}
    return `monitor-${index}`;
}

function isMaximized(w) {
    if (typeof w.is_maximized === 'function')
        return w.is_maximized();
    return w.get_maximized() === Meta.MaximizeFlags.BOTH;
}

function unmaximize(w) {
    try {
        w.unmaximize(Meta.MaximizeFlags.BOTH);
    } catch (e) {
        w.unmaximize();
    }
}

function maximize(w) {
    try {
        w.maximize(Meta.MaximizeFlags.BOTH);
    } catch (e) {
        w.maximize();
    }
}

// A window Mutter has not placed yet has an empty frame: it is not on screen.
function placed(w) {
    const r = w.get_frame_rect();
    return r.width > 0 && r.height > 0;
}

function appWindows() {
    return global.get_window_actors()
        .map(a => a.get_meta_window())
        .filter(w => w && w.get_window_type() === Meta.WindowType.NORMAL &&
            !w.is_skip_taskbar() && !w.get_transient_for());
}

class Service {
    get Version() {
        return 1;
    }

    SnapshotAsync(params, invocation) {
        guarded(invocation, () => this._snapshot());
    }

    PlaceAsync([json], invocation) {
        guarded(invocation, () => this._place(json));
    }

    _snapshot() {
        const display = global.display;
        const workspaces = global.workspace_manager;
        const tracker = Shell.WindowTracker.get_default();
        const out = {outputs: [], desktops: [], current: null, windows: []};
        for (let i = 0; i < display.get_n_monitors(); i++)
            out.outputs.push({name: monitorName(i), rect: rect(display.get_monitor_geometry(i)), index: i === display.get_primary_monitor() ? 0 : i + 1});
        for (let i = 0; i < workspaces.get_n_workspaces(); i++)
            out.desktops.push({id: String(i), name: Meta.prefs_get_workspace_name(i), index: i});
        out.current = String(workspaces.get_active_workspace_index());
        for (const w of appWindows().filter(placed)) {
            const app = tracker.get_window_app(w);
            const appId = app && !app.is_window_backed() ? app.get_id() : '';
            let state = 'normal';
            if (w.is_fullscreen())
                state = 'fullscreen';
            else if (w.minimized)
                state = 'minimized';
            else if (isMaximized(w))
                state = 'maximized';
            const ws = w.is_on_all_workspaces() ? null : w.get_workspace();
            out.windows.push({
                handle: String(w.get_id()),
                class: w.get_wm_class() ?? '',
                instance: w.get_wm_class_instance() ?? '',
                desktop_file: appId || (w.get_gtk_application_id() ?? ''),
                pid: w.get_pid() > 0 ? w.get_pid() : 0,
                title: w.get_title() ?? '',
                rect: rect(w.get_frame_rect()),
                state,
                output: w.get_monitor() >= 0 ? monitorName(w.get_monitor()) : null,
                desktop: ws ? String(ws.index()) : null,
            });
        }
        return JSON.stringify(out);
    }

    _place(json) {
        const ops = JSON.parse(json);
        if (!Array.isArray(ops))
            throw new Error('expected a list of placements');
        const byId = new Map(appWindows().map(w => [String(w.get_id()), w]));
        const outputs = [];
        for (let i = 0; i < global.display.get_n_monitors(); i++)
            outputs.push(monitorName(i));
        const results = [];
        for (const op of ops) {
            const w = byId.get(String(op.handle));
            if (!w) {
                results.push({handle: String(op.handle), ok: false, note: 'window closed'});
                continue;
            }
            // Mutter places a new window when it is first shown, over any
            // earlier move: wait for that moment.
            if (!placed(w)) {
                const id = w.connect('shown', () => {
                    w.disconnect(id);
                    this._apply(w, op, outputs);
                });
                results.push({handle: String(op.handle), ok: true, note: null});
                continue;
            }
            results.push(this._apply(w, op, outputs));
        }
        return JSON.stringify(results);
    }

    _apply(w, op, outputs) {
        try {
            // Changing to the same workspace or monitor would drop the
            // move that follows: only act on a real change.
            if (op.desktop !== null && op.desktop !== undefined) {
                const ws = global.workspace_manager.get_workspace_by_index(Number(op.desktop));
                if (ws && (w.is_on_all_workspaces() || w.get_workspace() !== ws))
                    w.change_workspace(ws);
            }
            if (w.is_fullscreen() && op.state !== 'fullscreen')
                w.unmake_fullscreen();
            if (w.minimized && op.state !== 'minimized')
                w.unminimize();
            if (isMaximized(w) && op.state !== 'maximized')
                unmaximize(w);
            const monitor = op.output ? outputs.indexOf(op.output) : -1;
            if (monitor >= 0 && w.get_monitor() !== monitor)
                w.move_to_monitor(monitor);
            let moved = true;
            if (op.rect) {
                if (w.allows_move() && w.allows_resize())
                    w.move_resize_frame(true, op.rect.x, op.rect.y, op.rect.width, op.rect.height);
                else if (w.allows_move())
                    w.move_frame(true, op.rect.x, op.rect.y);
                else
                    moved = false;
            }
            if (op.state === 'maximized')
                maximize(w);
            else if (op.state === 'fullscreen')
                w.make_fullscreen();
            else if (op.state === 'minimized')
                w.minimize();
            return {handle: String(op.handle), ok: moved, note: moved ? null : 'this window cannot be moved'};
        } catch (e) {
            return {handle: String(op.handle), ok: false, note: String(e)};
        }
    }
}

export default class LyapunovExtension extends Extension {
    enable() {
        this._object = Gio.DBusExportedObject.wrapJSObject(INTERFACE, new Service());
        this._object.export(Gio.DBus.session, OBJECT_PATH);
        this._owner = Gio.bus_own_name_on_connection(Gio.DBus.session, BUS_NAME,
            Gio.BusNameOwnerFlags.NONE, null, null);
    }

    disable() {
        if (this._owner) {
            Gio.bus_unown_name(this._owner);
            this._owner = 0;
        }
        this._object?.unexport();
        this._object = null;
    }
}
