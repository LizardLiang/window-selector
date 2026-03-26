# Window Selector

A fast, keyboard-driven Alt+Tab replacement for Windows. Press a hotkey to see live DWM thumbnails of all open windows in a fullscreen grid, then hit a letter key to select and switch.

## Features

- **Live DWM Thumbnails** — Real-time window previews rendered by the Desktop Window Manager, not static screenshots.
- **Keyboard-First Navigation** — Each window is assigned a home-row-first letter (A, S, D, F, G, H, J, K, ...). Press the letter to select, Enter/Space to switch.
- **Number Tagging** — Pin frequently used windows with Ctrl+1 through Ctrl+9. Press the number key to jump back instantly.
- **Quick List Bar** — A compact strip at the bottom of the overlay showing all windows with their letter, number tag, and title at a glance.
- **Multi-Monitor Support** — Overlay spans all connected monitors with per-monitor grid layout.
- **MRU Ordering** — Windows are sorted by most-recently-used order, tracked in real time via a WinEvent hook.
- **Accent Color Integration** — Selection highlight uses the Windows system accent color.
- **Aura Glow Effect** — Selected cells have a 3-layer bloom glow for clear visual feedback.
- **System Tray** — Runs in the background with a tray icon. Right-click for Settings, About, or Exit.
- **Configurable Hotkey** — Default is Ctrl+Alt+Space. Change it in the Settings dialog or edit the config file.

## Requirements

- Windows 10/11 (x86_64)
- Visual Studio 2022 Build Tools (MSVC toolchain)
- Rust (stable)

## Build

```bash
cargo build              # Debug build
cargo build --release    # Release build (LTO enabled)
```

The target triple is hardcoded to `x86_64-pc-windows-msvc` in `.cargo/config.toml`.

## Run

```bash
cargo run                # Debug
cargo run --release      # Release
```

The application starts minimized to the system tray. Press the activation hotkey (default: **Ctrl+Alt+Q**) to open the overlay, or **Win+Y** for label mode.

## Usage

| Action | Key |
|--------|-----|
| Open overlay (full-screen grid) | Ctrl+Alt+Q (configurable) |
| Open label mode (quick labels) | Win+Y (configurable) |
| Select a window | Press the letter shown on its thumbnail/label |
| Switch to selected window | Enter or Space |
| Dismiss overlay | Escape |
| Tag a window with a number | Ctrl+1 through Ctrl+9 (while a window is selected) |
| Jump to a tagged window | 1 through 9 |

### Letter Assignment

Letters are assigned in an ergonomic home-row-first order:

```
a s d f g h j k l    (home row)
q w e r t            (top row left)
y u i o p            (top row right)
z x c v b n m        (bottom row)
```

The most recently used window gets **A**, the second gets **S**, and so on. Up to 26 windows can receive a letter.

## Configuration

Config file: `%APPDATA%\window-selector\config.toml`

```toml
hotkey_modifiers = 16387   # MOD_WIN | MOD_NOREPEAT
hotkey_vk = 81             # VK_Q (Ctrl+Alt+Q for overlay)
label_hotkey_modifiers = 16387   # MOD_WIN | MOD_NOREPEAT
label_hotkey_vk = 89             # VK_Y (Win+Y for label mode)
```

Logs are written to `%APPDATA%\window-selector\logs\`.

## Architecture

Single-threaded Win32 message pump. All state lives on the main thread — no async runtime, no multi-threading.

| Module | Purpose |
|--------|---------|
| `main.rs` | Entry point, message loop, window procedures |
| `state.rs` | `OverlayState` state machine (Hidden/FadingIn/Active/FadingOut) |
| `overlay.rs` | Overlay window management, show/hide with fade animation |
| `overlay_renderer.rs` | Direct2D + DirectWrite rendering (backdrop, cells, glow, quick list) |
| `dwm_thumbnails.rs` | DWM live thumbnail registration and letterboxing |
| `grid_layout.rs` | Aspect-ratio-driven grid computation |
| `interaction.rs` | Keyboard input handling (pure logic, returns `KeyAction` enum) |
| `window_enumerator.rs` | Window enumeration with Alt+Tab heuristic filters |
| `letter_assignment.rs` | Home-row-first letter sequence |
| `mru_tracker.rs` | Real-time MRU tracking via `EVENT_SYSTEM_FOREGROUND` |
| `window_switcher.rs` | Focus transfer with `AllowSetForegroundWindow` + fallback |
| `animation.rs` | Fade animator (80ms, ~60fps) |
| `config.rs` | TOML config with atomic writes |
| `hotkey.rs` | Global hotkey registration |
| `tray.rs` | System tray icon and context menu |
| `monitor.rs` | Multi-monitor enumeration |
| `accent_color.rs` | Windows system accent color reader |
| `settings_dialog.rs` | Settings dialog (Win32) |
| `about_dialog.rs` | About dialog (Win32) |

## License

MIT

---

# Window Selector

一款快速、鍵盤導向的 Windows Alt+Tab 替代工具。按下快捷鍵即可看到所有開啟視窗的 DWM 即時縮圖方格，再按對應字母鍵即可選取並切換視窗。

## 功能特色

- **DWM 即時縮圖** — 由桌面視窗管理員即時繪製的視窗預覽，不是靜態截圖。
- **鍵盤優先操作** — 每個視窗依人體工學 Home Row 順序分配字母（A、S、D、F、G、H、J、K……），按字母選取，Enter/Space 切換。
- **數字標籤** — 用 Ctrl+1 到 Ctrl+9 釘選常用視窗，按數字鍵即可快速跳回。
- **快速列表欄** — 覆蓋層底部的精簡橫條，一覽所有視窗的字母、數字標籤與標題。
- **多螢幕支援** — 覆蓋層橫跨所有已連接的螢幕，每個螢幕獨立計算方格配置。
- **MRU 排序** — 視窗依最近使用順序排列，透過 WinEvent Hook 即時追蹤。
- **系統強調色整合** — 選取高亮使用 Windows 系統強調色。
- **光暈效果** — 選取的方格有三層光暈綻放效果，提供清晰的視覺回饋。
- **系統匣** — 在背景執行，右鍵系統匣圖示可開啟設定、關於或結束程式。
- **可自訂快捷鍵** — 預設為 Ctrl+Alt+Space，可在設定對話框或設定檔中修改。

## 系統需求

- Windows 10/11（x86_64）
- Visual Studio 2022 Build Tools（MSVC 工具鏈）
- Rust（stable）

## 建置

```bash
cargo build              # Debug 建置
cargo build --release    # Release 建置（啟用 LTO）
```

目標三元組固定為 `x86_64-pc-windows-msvc`，定義於 `.cargo/config.toml`。

## 執行

```bash
cargo run                # Debug
cargo run --release      # Release
```

程式啟動後會最小化至系統匣。按下啟動快捷鍵（預設：**Ctrl+Alt+Q** 開啟覆蓋層，**Win+Y** 開啟標籤模式）。

## 使用方式

| 操作 | 按鍵 |
|------|------|
| 開啟覆蓋層（全螢幕方格） | Ctrl+Alt+Q（可自訂） |
| 開啟標籤模式（快速標籤） | Win+Y（可自訂） |
| 選取視窗 | 按縮圖/標籤上顯示的字母 |
| 切換至選取的視窗 | Enter 或 Space |
| 關閉覆蓋層 | Escape |
| 為視窗加上數字標籤 | Ctrl+1 到 Ctrl+9（需先選取視窗） |
| 跳至已標籤的視窗 | 1 到 9 |

### 字母分配順序

字母依人體工學 Home Row 優先順序分配：

```
a s d f g h j k l    （Home Row）
q w e r t            （上排左側）
y u i o p            （上排右側）
z x c v b n m        （下排）
```

最近使用的視窗分配 **A**，第二個分配 **S**，以此類推。最多可分配 26 個字母。

## 設定

設定檔位置：`%APPDATA%\window-selector\config.toml`

```toml
hotkey_modifiers = 16387   # MOD_WIN | MOD_NOREPEAT
hotkey_vk = 81             # VK_Q（Ctrl+Alt+Q 開啟覆蓋層）
label_hotkey_modifiers = 16387   # MOD_WIN | MOD_NOREPEAT
label_hotkey_vk = 89             # VK_Y（Win+Y 開啟標籤模式）
```

日誌寫入 `%APPDATA%\window-selector\logs\`。

## 架構

單執行緒 Win32 訊息迴圈。所有狀態存在主執行緒——無非同步執行時、無多執行緒。

| 模組 | 用途 |
|------|------|
| `main.rs` | 程式進入點、訊息迴圈、視窗程序 |
| `state.rs` | `OverlayState` 狀態機（Hidden/FadingIn/Active/FadingOut） |
| `overlay.rs` | 覆蓋層視窗管理、顯示/隱藏與淡入淡出動畫 |
| `overlay_renderer.rs` | Direct2D + DirectWrite 繪製（背景、方格、光暈、快速列表） |
| `dwm_thumbnails.rs` | DWM 即時縮圖註冊與信箱式縮放 |
| `grid_layout.rs` | 依長寬比計算方格配置 |
| `interaction.rs` | 鍵盤輸入處理（純邏輯，回傳 `KeyAction` 列舉） |
| `window_enumerator.rs` | 視窗列舉，套用 Alt+Tab 啟發式篩選 |
| `letter_assignment.rs` | Home Row 優先字母序列 |
| `mru_tracker.rs` | 透過 `EVENT_SYSTEM_FOREGROUND` 即時追蹤 MRU |
| `window_switcher.rs` | 以 `AllowSetForegroundWindow` + 備援方式轉移焦點 |
| `animation.rs` | 淡入淡出動畫器（80ms、~60fps） |
| `config.rs` | TOML 設定檔，原子寫入 |
| `hotkey.rs` | 全域快捷鍵註冊 |
| `tray.rs` | 系統匣圖示與右鍵選單 |
| `monitor.rs` | 多螢幕列舉 |
| `accent_color.rs` | 讀取 Windows 系統強調色 |
| `settings_dialog.rs` | 設定對話框（Win32） |
| `about_dialog.rs` | 關於對話框（Win32） |

## 授權

MIT
