---
name: run-settings-panel
description: Launch window-selector and drive its Win32 settings panel — click sidebar pages, record keybindings, capture window screenshots. Use when verifying settings/overlay UI changes in the real app rather than only in tests, or when asked to run, screenshot, or click through the app.
---

# Driving the window-selector settings panel

This app is a **raw Win32 GUI** (Direct2D-rendered, tray-resident). No
Playwright, no `_electron`, no accessibility tree — the only way to
drive it is Win32 P/Invoke against real HWNDs. `drive.ps1` in this
directory is a verified driver; source it rather than rewriting it.

Unit tests cannot reach this surface: hit-testing, layout clipping, and
label text are all computed in `settings_renderer.rs` at draw time.
Screenshots are the only proof. **Look at the screenshot** — a blank or
clipped frame is a failure.

## Build first

Local builds use the **gnu** triple (`.cargo/config.toml`), not msvc:

```bash
cargo build --release        # -> target/x86_64-pc-windows-gnu/release/window-selector.exe
```

`cargo build` alone is a *debug* build and will not reflect the release
binary. Building `--target x86_64-pc-windows-msvc` locally fails at the
link step on this machine; that triple is CI-only.

## Protect the user's config before launching

The app persists to `%APPDATA%\window-selector\config.toml` and mutates
it on every settings change. Always back up and restore:

```powershell
$cfg = "$env:APPDATA\window-selector\config.toml"
Copy-Item $cfg "$cfg.bak" -Force            # before
# ... drive the app ...
Stop-Process -Name window-selector -Force   # kill FIRST, or it rewrites config
Move-Item "$cfg.bak" $cfg -Force            # then restore
```

## Opening the settings window without clicking the tray

Deleting `config.toml` makes `guide_shown` unset, and first-run
auto-opens the settings window **on the Guide page**. That is the
cheapest way in — no tray automation needed, and it exercises the
first-run path for free.

```powershell
Remove-Item "$env:APPDATA\window-selector\config.toml" -Force
Start-Process "target\x86_64-pc-windows-gnu\release\window-selector.exe"
```

Otherwise the window opens on the **Keybindings** page (the default).

## Using the driver

```powershell
. .claude\skills\run-settings-panel\drive.ps1

$h = Get-SettingsHwnd -TimeoutSec 12          # finds class WindowSelectorSettings
Click-Client -Hwnd $h -X 70 -Y 42             # client coords, mapped to screen for you
Send-Key -Vk 0x4A -Modifiers @(0x11)          # Ctrl+J
Shot -Hwnd $h -Path shot.png                  # window-only capture, not whole desktop
```

`Get-SettingsHwnd` returns `[IntPtr]::Zero` on timeout — check it.

## Client coordinate map

`Click-Client` takes **client** coordinates and maps them to screen, so
these come straight from the constants in `settings_renderer.rs`. Client
area is exactly 620x560 (`AdjustWindowRectEx` guarantees it). Re-derive
these if `PANEL_WIDTH`, `SIDEBAR_WIDTH`, or the page layout changes.

Sidebar (`left=12, right=128, item_h=44, gap=6, start_y=20`) — click x=70:

| Page | y range | click y |
|---|---|---|
| Keybindings | 20–64 | 42 |
| Behavior | 70–114 | 92 |
| Appearance | 120–164 | 142 |
| Guide | 170–214 | 192 |

Keybindings page fields (`control_left=344, content_right=596`) — click x=470:

| Control | y range | click y |
|---|---|---|
| Main Overlay hotkey | 50–80 | 65 |
| Label Mode hotkey | 90–120 | 105 |
| Confirm | 170–200 | 185 |
| Dismiss | 210–240 | 225 |

Modifier buttons span `content_left=164` to `596` in 4 columns with an
8px gap (≈55px each): Ctrl / Alt / Shift / Win. Tag-assign row y≈296–324,
move-to-monitor row y≈366–394.

Footer `Reset to Defaults`: y 500–536, centered.

## Gotchas that cost real time

- **`FindWindowW($class, $null)` always returns 0.** PowerShell marshals
  `$null` into a `string` P/Invoke parameter as `""`, so it searches for
  a window with an *empty* title. Use `[NullString]::Value`. The driver
  already does.
- **The first click on a freshly-opened or unfocused window is eaten by
  activation** and does nothing. This looks exactly like a hit-testing
  bug — a sidebar click that leaves the page unchanged. Either click the
  same spot twice, or throw away one warm-up click after
  `Get-SettingsHwnd`, before trusting any result. Verify a suspected
  layout bug by re-clicking on the already-focused window before
  reporting it.
- **Don't click "Reset to Defaults" casually.** It writes the
  launch-at-startup **registry key** to false, changing a real user
  setting. Restore the config file instead.
- **The keybinding recorder installs a global `WH_KEYBOARD_LL` hook** and
  swallows keystrokes while recording. Synthetic `keybd_event` input is
  captured fine. If a run aborts mid-recording, send Escape (VK 0x1B) or
  kill the process — otherwise your keyboard stays swallowed.
- **Screenshot the window, not the desktop.** `Shot` uses
  `GetClientRect` + `ClientToScreen` so the image is exactly the client
  area, with no desktop clutter and no caption offset to reason about.
- **`Get-Process window-selector` is how you confirm launch.**
  `tasklist /FI` prints a localized "no tasks" message that is easy to
  misread as output.
- Logs land in `%APPDATA%\window-selector\logs\window-selector.<date>.log`
  and record settings-panel open with its HWND — check there first when
  the window seems absent.

## Verified expectations

Useful as regression anchors:

- Client area measures 620x560; the footer button is fully visible on
  every page.
- All four pages render their own controls: Keybindings (GLOBAL HOTKEYS
  + IN-OVERLAY KEYS), Behavior (two toggles + LABEL MODE overlap
  buttons), Appearance (six sliders with value readouts), Guide.
- Guide renders 8 rows, all from live config. Rebinding Confirm to
  `Ctrl+J` makes the Guide read `Ctrl+J or Space` — it must never show
  hardcoded keys.
- Dismiss field shows plain `Esc`, not `Esc (or Esc)`.
- Recorder on Confirm/Dismiss: bare letter → "Letters need a modifier";
  any digit, modifier or not → "Digits are reserved for tags"; `Ctrl+J`
  → accepted, persists `confirm_modifiers = 2, confirm_vk = 74`. A
  rejection must leave the prior binding untouched in `config.toml`.
