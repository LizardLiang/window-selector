# Window Selector — Windows 鍵盤驅動的 Alt+Tab 替代工具

[English README](README.md)

一款快速、零設定的 Windows 10/11 Alt+Tab 替代工具。按下快捷鍵即可看到所有開啟視窗的 DWM 即時縮圖方格，再按對應字母鍵即可選取並切換視窗。以 Rust 搭配 Direct2D 開發。

[![平台](https://img.shields.io/badge/platform-Windows%2010%2F11-blue)](https://github.com/LizardLiang/window-selector)
[![授權](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![版本](https://img.shields.io/github/v/release/LizardLiang/window-selector)](https://github.com/LizardLiang/window-selector/releases/latest)
[![語言](https://img.shields.io/badge/language-Rust-orange?logo=rust)](https://www.rust-lang.org/)

## 目錄

- [示範](#示範)
- [功能特色](#功能特色)
- [為什麼選擇 Window Selector？](#為什麼選擇-window-selector)
- [下載與安裝](#下載與安裝)
- [建置](#建置)
- [執行](#執行)
- [使用方式](#使用方式)
- [設定](#設定)
- [架構](#架構)
- [貢獻](#貢獻)
- [授權](#授權)

## 示範

<!-- TODO: 新增 docs/screenshot.png 截圖後取消註解：
![Window Selector 覆蓋層示範](docs/screenshot.png)
全螢幕覆蓋層，顯示 DWM 即時縮圖、字母標籤，以及底部的快速列表欄。
-->

*截圖即將推出 — 字母選取流程的示範 GIF 正在製作中。*

## 功能特色

- **DWM 即時縮圖** — 由桌面視窗管理員即時繪製的視窗預覽，不是靜態截圖。
- **鍵盤優先操作** — 每個視窗依人體工學 Home Row 順序分配字母（A、S、D、F、G、H、J、K……），按字母選取，Enter/Space 切換。
- **數字標籤** — 用 Ctrl+1 到 Ctrl+9 釘選常用視窗。標籤跨重啟持久保存，並自動解析至最近使用的對應視窗。
- **快速列表欄** — 覆蓋層底部的精簡橫條，一覽所有視窗的字母、數字標籤與標題。
- **多螢幕支援** — 覆蓋層橫跨所有已連接的螢幕，每個螢幕獨立計算方格配置。
- **MRU 排序** — 視窗依最近使用順序排列，透過 WinEvent Hook 即時追蹤。
- **系統強調色整合** — 選取高亮使用 Windows 系統強調色。
- **光暈效果** — 選取的方格有三層光暈綻放效果，提供清晰的視覺回饋。
- **系統匣** — 在背景執行，右鍵系統匣圖示可開啟設定、關於或結束程式。
- **可自訂快捷鍵** — 預設為 Ctrl+Alt+Q，可在設定對話框或設定檔中修改。

## 為什麼選擇 Window Selector？

Windows 內建的 Alt+Tab 需要逐一循環所有開啟的視窗，沒有辦法直接跳至指定視窗。Window Selector 以全螢幕方格取代這個操作流程，每個視窗都有字母標籤：按下字母，再按 Enter 或 Space 確認，即可完成切換。不需要滑鼠，不需要循環翻找。

| 工具 | 字母導覽 | 即時縮圖 | 多螢幕 | 持久標籤 | 開源 |
|------|:--------:|:--------:|:------:|:--------:|:----:|
| Windows 內建 Alt+Tab | — | — | — | — | — |
| PowerToys（無對應功能） | — | — | — | — | ✓ |
| TaskSwitchXP | — | — | — | — | — |
| **Window Selector** | ✓ | ✓ | ✓ | ✓ | ✓ |

Window Selector 是 Windows 10/11 上唯一免費、開源、單一執行檔，且同時支援 Home Row 字母導覽與持久數字標籤的 Alt+Tab 替代工具。

## 下載與安裝

請從[最新 GitHub Release](https://github.com/LizardLiang/window-selector/releases/latest) 下載預建執行檔：

- **免安裝版** — `window-selector.exe`：無需安裝，下載即用。
- **安裝版** — `WindowSelector-<version>-setup.exe`：NSIS 安裝程式，含「開始功能表」整合。

免安裝版快速上手：

1. 從 Releases 頁面下載 `window-selector.exe`。
2. 將檔案放置於任意位置，無需安裝。
3. 雙擊執行，程式啟動後會最小化至系統匣。
4. 按下 **Ctrl+Alt+Q** 開啟視窗切換覆蓋層。

> **Windows Defender SmartScreen 提示：** 由於執行檔尚未進行程式碼簽章，Windows 首次執行時可能顯示 SmartScreen 警告。請點選**更多資訊 → 仍要執行**繼續。本儲存庫提供完整原始碼供檢視。


## 建置

**系統需求：**
- Windows 10/11（x86_64）
- Visual Studio 2022 Build Tools（MSVC 工具鏈）
- Rust stable 工具鏈

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

程式啟動後會最小化至系統匣。按下啟動快捷鍵（預設：**Ctrl+Alt+Q**）開啟覆蓋層，或按 **Win+Y** 開啟標籤模式。

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
| 將選取的視窗移到指定螢幕中央 | Shift+1 到 Shift+9（需先選取視窗；覆蓋層會在每個螢幕上顯示對應數字） |

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
hotkey_modifiers = 16387   # MOD_CONTROL | MOD_ALT | MOD_NOREPEAT
hotkey_vk = 81             # VK_Q（Ctrl+Alt+Q 開啟覆蓋層）
label_hotkey_modifiers = 16392   # MOD_WIN | MOD_NOREPEAT
label_hotkey_vk = 89             # VK_Y（Win+Y 開啟標籤模式）

[[quick_tags]]
number = 1
exe_path = 'C:\\Windows\\System32\\notepad.exe'
```

日誌寫入 `%APPDATA%\window-selector\logs\`。

## 架構

單執行緒 Win32 訊息迴圈。所有狀態存在主執行緒——無非同步執行時、無多執行緒。鍵盤驅動的視窗切換器以 Rust 開發，使用 `windows` crate 進行 Win32 綁定、Direct2D 渲染，並透過 DWM 取得即時縮圖。

<details>
<summary>模組說明</summary>

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

</details>

## 貢獻

歡迎提交 Issue 與 Pull Request。程式碼庫為單執行緒 Win32 架構；提交修補前請先閱讀 `CLAUDE.md` 了解架構說明。新功能建議先開 Issue 討論實作方向。

## 授權

MIT
