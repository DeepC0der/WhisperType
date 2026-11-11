# Linux Port Architecture & Implementation Roadmap (`LINUX_TODO.md`)

This document outlines the technical architecture, OS-level differences, dependency candidates, and implementation tasks required to port Murmur to GNU/Linux environments.

---

## 1. Architectural Differences: Windows vs. Linux

| Subsystem | Windows Implementation (Phase 1) | Linux Challenge & Target Architecture |
|---|---|---|
| **Global Hotkeys** | `WH_KEYBOARD_LL` Win32 hook on dedicated thread with message pump | **Wayland vs. X11 fragmentation:** Wayland does not permit background applications from snooping global keystrokes. Must use `org.freedesktop.portal.GlobalShortcuts` (XDG Desktop Portal) or direct `evdev` (`/dev/input/event*`) with `uinput` group permissions. For legacy X11, `XGrabKey` / `xcb`. |
| **Text Injection** | Dual mode: `SendInput` clipboard paste (`Ctrl+V` with restore) + `KEYEVENTF_UNICODE` fallback | **Wayland:** Requires virtual input via `/dev/uinput` (using `ydotool` or `evdev` crate) or `wtype` (wlroots-only). Clipboard paste via Wayland data-control protocol or `wl-clipboard`. **X11:** `xdotool` or `XTestFakeKeyEvent` via `x11rb`. |
| **Overlay Window** | Frameless, transparent, `WS_EX_NOACTIVATE \| WS_EX_TOOLWINDOW \| WS_EX_LAYERED \| WS_EX_TRANSPARENT` | **Wayland:** Traditional windows cannot position themselves or float on top arbitrarily. Must use `wlr-layer-shell` (`gtk-layer-shell` bindings) or GNOME shell extension / KWin script for overlay placement. **X11:** Override-redirect window or `_NET_WM_WINDOW_TYPE_DOCK` / `_NET_WM_STATE_STAYS_ON_TOP`. |
| **Autostart** | Windows Registry (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`) | XDG Autostart specification: Create desktop entry in `$XDG_CONFIG_HOME/autostart/murmur.desktop` (defaults to `~/.config/autostart/`). |
| **Audio Capture** | `cpal` WASAPI host | `cpal` ALSA or PipeWire/PulseAudio host. Ensure low-latency stream buffer configuration. |
| **ASR Engine** | `sherpa-onnx` shared C-API DLLs (`onnxruntime.dll`) | `sherpa-onnx` prebuilt Linux `.so` shared libraries (`libsherpa-onnx-c-api.so`, `libonnxruntime.so`). Standard rpath/LD_LIBRARY_PATH resolution. |

---

## 2. Linux Trait Implementation Plan

### 2.1. `HotkeyBackend` (`src-tauri/src/platform/linux/hotkey.rs`)
- **Primary (Wayland Desktop Portal):**
  - Integrate with `ashpd` (Rust bindings for XDG Desktop Portals) targeting `org.freedesktop.portal.GlobalShortcuts`.
  - Solicits user approval once to register shortcuts (e.g. `Ctrl+Shift+Space`).
- **Secondary (Direct evdev / Rootless uinput):**
  - Use `evdev` crate reading from `/dev/input/event*`.
  - Requires user to be in `input` group or udev rule (`/etc/udev/rules.d/99-murmur-input.rules`).
- **Fallback (X11):**
  - Use `x11rb` to bind global hotkey with `XGrabKey` on the root window.

### 2.2. `TextInjector` (`src-tauri/src/platform/linux/inject.rs`)
- **Clipboard Paste Emulation:**
  - Save clipboard via `wl-clipboard` (`wl-paste`) or `arboard` (X11/Wayland support enabled).
  - Copy transcribed text to clipboard.
  - Synthesize `Ctrl+Shift+V` or `Ctrl+V` using `uinput` virtual keyboard device.
  - Sleep 120ms.
  - Restore previous clipboard contents.
- **Direct Keystroke Typing:**
  - Emit key press / release events via `/dev/uinput` using `uinput` or `evdev` crate.

### 2.3. `OverlayWindowExt` (`src-tauri/src/platform/linux/window.rs`)
- **X11:**
  - Set window properties via X11 atoms:
    ```c
    _NET_WM_WINDOW_TYPE = _NET_WM_WINDOW_TYPE_DOCK
    _NET_WM_STATE = _NET_WM_STATE_ABOVE | _NET_WM_STATE_STICKY
    ```
  - Enable input passthrough via XShape extension (`XShapeCombineRectangles` with empty region).
- **Wayland:**
  - Hook into GTK/WebKitGTK underlying window to invoke `gtk_layer_init_for_window()`, setting layer to `GTK_LAYER_SHELL_LAYER_OVERLAY` with keyboard mode `NONE`.

### 2.4. `Autostart` (`src-tauri/src/platform/linux/autostart.rs`)
- Write an XDG desktop entry file to `$HOME/.config/autostart/murmur.desktop`:
  ```ini
  [Desktop Entry]
  Type=Application
  Name=Murmur
  Exec=/usr/bin/murmur
  Hidden=false
  NoDisplay=false
  X-GNOME-Autostart-enabled=true
  Comment=Offline voice dictation tool
  ```

---

## 3. Required Linux Build Dependencies & Toolchains

1. **System Libraries (Debian/Ubuntu):**
   ```bash
   sudo apt-get install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev
   ```
2. **Wayland Development Packages:**
   ```bash
   sudo apt-get install libwayland-dev libgtk-layer-shell-dev
   ```
3. **Packaging:**
   - AppImage (`cargo tauri build --bundles appimage`)
   - Debian `.deb` package
   - Flatpak (with portal permissions for Audio, Notifications, and GlobalShortcuts)
