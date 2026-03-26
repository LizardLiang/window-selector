# 標籤模式 (Label Mode) 功能說明

## 🎯 功能概述

標籤模式是一個全新的視窗切換方式，靈感來自 [FastWindowSwitcher](https://github.com/JochenBaier/fastwindowswitcher)。

與全螢幕覆蓋層不同，標籤模式會在每個視窗的標題列上方直接顯示小型標籤，讓你可以快速識別並跳轉到目標視窗。

## ⌨️ 快捷鍵

| 功能 | 快捷鍵 | 說明 |
|------|--------|------|
| 啟動標籤模式 | `Win+Y` | 在所有視窗上方顯示字母標籤 |
| 選擇視窗 | `A-Z` | 按對應字母選擇視窗（會高亮顯示） |
| 跳轉視窗 | `Enter` 或 `Space` | 切換到選中的視窗 |
| 取消 | `Esc` | 關閉標籤模式 |

## 🎨 視覺設計

### 標籤樣式
- **現代風格**: 使用 Windows 系統強調色
- **光暈效果**: 選中的標籤有三層光暈綻放效果
- **半透明背景**: 深色背景（95% 不透明度）
- **大字體**: 24px Segoe UI Bold

### 標籤位置
- 位於視窗標題列**正上方** 52 像素處
- 水平居中對齊
- 尺寸: 60x48 像素

### 字母分配順序
與主覆蓋層相同，採用人體工學 Home Row 優先順序：

```
a s d f g h j k l    (Home Row - 最常用)
q w e r t            (上排左側)
y u i o p            (上排右側)
z x c v b n m        (下排)
```

最近使用的視窗獲得 **A**，第二個獲得 **S**，以此類推。

## 🔧 技術實作

### 新增檔案
1. **`src/label_overlay.rs`** - 標籤視窗管理器
   - `LabelWindow`: 單一標籤視窗
   - `LabelOverlayManager`: 管理所有標籤視窗

2. **`src/label_renderer.rs`** - Direct2D 繪製
   - 使用 Direct2D 繪製標籤
   - 支援光暈效果和系統強調色

### 修改檔案
1. **`src/config.rs`**
   - 新增 `label_hotkey_modifiers` 和 `label_hotkey_vk`
   - 預設值: `Win+O` (MOD_WIN | MOD_NOREPEAT, VK_O)

2. **`src/state.rs`**
   - 新增 `OverlayState::LabelMode { selected: Option<usize> }`
   - 新增 `is_label_mode()` 方法

3. **`src/hotkey.rs`**
   - 新增 `HOTKEY_ID_LABEL = 2`
   - 新增 `register_label_hotkey()` 和 `unregister_label_hotkey()`

4. **`src/main.rs`**
   - 新增 `label_overlay_manager: LabelOverlayManager`
   - 新增 `handle_label_hotkey()`, `activate_label_mode()`, `dismiss_label_mode()`
   - 修改 `handle_overlay_key()` 支援標籤模式
   - 新增 `label_wndproc()` 視窗程序

5. **`src/interaction.rs`**
   - 修改 `handle_hotkey_event()` 和 `handle_focus_lost()` 處理 `LabelMode` 狀態

## 🚀 使用流程

### 典型使用場景

1. **快速切換到可見視窗**
   ```
   Win+Y → 看到所有視窗上方的字母標籤 → 按 F → 立即跳轉
   ```

2. **選擇後確認**
   ```
   Win+Y → 按 D 選擇 → 標籤高亮顯示 → Enter 確認跳轉
   ```

3. **取消操作**
   ```
   Win+Y → 看到標籤 → Esc 取消
   ```

## 🆚 與全螢幕覆蓋層的比較

| 特性 | 標籤模式 (Win+Y) | 全螢幕覆蓋層 (Ctrl+Alt+Q) |
|------|------------------|----------------------|
| 視覺呈現 | 小型標籤在視窗上方 | 全螢幕方格縮圖 |
| 視窗預覽 | 無（直接看原視窗） | DWM 即時縮圖 |
| 適用場景 | 視窗可見時快速切換 | 視窗被遮擋或最小化 |
| 視覺干擾 | 最小 | 中等（全螢幕） |
| 操作速度 | 極快 | 快 |
| 多螢幕支援 | 是 | 是 |

## 📝 設定檔

標籤模式快捷鍵儲存在 `%APPDATA%\window-selector\config.toml`:

```toml
# 主覆蓋層快捷鍵 (Ctrl+Alt+Q)
hotkey_modifiers = 16387   # MOD_WIN | MOD_NOREPEAT
hotkey_vk = 81             # VK_Q

# 標籤模式快捷鍵 (Win+Y)
label_hotkey_modifiers = 16387   # MOD_WIN | MOD_NOREPEAT
label_hotkey_vk = 89               # VK_Y
```

## 🐛 已知限制

1. **視窗移動**: 標籤位置在啟動時計算，視窗移動後標籤不會跟隨
2. **最小化視窗**: 最小化的視窗不會顯示標籤（因為沒有可見的標題列）
3. **多螢幕**: 標籤會顯示在所有螢幕上的視窗

## 🔮 未來改進

- [ ] 標籤跟隨視窗移動（需要監聽視窗位置變化）
- [ ] 可自訂標籤樣式（大小、顏色、字型）
- [ ] 支援數字標籤（與主覆蓋層的 Ctrl+1-9 整合）
- [ ] 動畫效果（淡入淡出）

## 📚 參考資料

- [FastWindowSwitcher](https://github.com/JochenBaier/fastwindowswitcher) - 靈感來源
- [Vimium](https://vimium.github.io/) - 瀏覽器標籤導航
- [VimFX](https://addons.mozilla.org/de/firefox/addon/vimfx/) - Firefox 標籤導航

---

**版本**: 0.1.0  
**最後更新**: 2026-03-26
