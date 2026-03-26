# 標籤模式問題解決方案

## 🔍 問題根本原因

經過詳細研究 FastWindowSwitcher 的原始碼，我發現了當前實作的根本問題：

### 當前實作（錯誤）❌
```
為每個視窗創建獨立的標籤視窗
→ 複雜、容易出錯
→ 標籤位置不準確
→ 鍵盤輸入路由困難
```

### FastWindowSwitcher 的實作（正確）✅
```
使用單一全螢幕透明覆蓋層
→ 在覆蓋層上繪製所有標籤
→ 簡單可靠
→ 與現有架構一致
```

## 🎯 新實作方案

### 核心概念

**不要創建多個標籤視窗，而是重用現有的覆蓋層！**

```
Win+Y → 顯示全螢幕透明覆蓋層 → 在每個視窗位置繪製標籤 → 按字母跳轉
```

### 實作步驟

#### 1. 修改 overlay_manager.rs

新增方法：
```rust
pub fn show_labels_only(&mut self, windows: &[WindowInfo]) {
    // 1. 顯示覆蓋層（透明背景）
    // 2. 不創建 DWM 縮圖
    // 3. 只繪製標籤
}
```

#### 2. 修改 overlay_renderer.rs

新增方法：
```rust
pub fn render_labels_only(&self, windows: &[WindowInfo], selected: Option<usize>) {
    // 清除為透明背景
    self.render_target.Clear(Some(&d2d_color(0.0, 0.0, 0.0, 0.0)));
    
    // 為每個視窗繪製標籤
    for (i, window) in windows.iter().enumerate() {
        let is_selected = Some(i) == selected;
        self.draw_label_for_window(window, is_selected);
    }
}

fn draw_label_for_window(&self, window: &WindowInfo, is_selected: bool) {
    // 1. 獲取視窗矩形
    let window_rect = get_window_rect(window.hwnd);
    
    // 2. 計算標籤位置（標題列上方）
    let label_pos = calculate_label_position(window_rect);
    
    // 3. 繪製標籤（類似現有的 badge 繪製）
    // 4. 如果選中，繪製光暈效果
}
```

#### 3. 標籤位置計算

```rust
fn calculate_label_position(window_rect: RECT) -> (f32, f32) {
    // 獲取視窗邊框寬度
    let border_width = unsafe {
        GetSystemMetrics(SM_CXSIZEFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER)
    } as f32;
    
    // 標籤在視窗中央上方
    let label_x = window_rect.left as f32 + 
                  (window_rect.right - window_rect.left) as f32 / 2.0 - 
                  LABEL_WIDTH / 2.0;
    
    // 標籤在標題列上方
    let label_y = window_rect.top as f32 + border_width;
    
    (label_x, label_y)
}
```

#### 4. 簡化 main.rs

```rust
unsafe fn activate_label_mode(app: &mut AppState) {
    // 獲取視窗列表
    app.window_snapshot = snapshot_windows(...);
    
    // 顯示覆蓋層（標籤模式）
    app.overlay_manager.show_labels_only(&app.window_snapshot);
    
    // 設定狀態
    app.overlay_state = OverlayState::LabelMode { selected: None };
    
    // 啟動鍵盤 hook
    keyboard_hook::set_active(true);
}
```

## 🔧 需要修改的檔案

### 1. 刪除不需要的檔案
- ❌ `src/label_overlay.rs` - 刪除（不需要獨立標籤視窗）
- ❌ `src/label_renderer.rs` - 刪除（使用現有 renderer）

### 2. 修改現有檔案
- ✅ `src/overlay_manager.rs` - 新增 `show_labels_only()`
- ✅ `src/overlay_renderer.rs` - 新增 `render_labels_only()`
- ✅ `src/main.rs` - 簡化 `activate_label_mode()`

### 3. 保持不變
- ✅ `src/config.rs` - 快捷鍵設定
- ✅ `src/state.rs` - 狀態定義
- ✅ `src/interaction.rs` - 鍵盤處理

## 📊 對比

| 項目 | 舊實作（多視窗） | 新實作（單覆蓋層） |
|------|------------------|-------------------|
| 視窗數量 | N 個標籤視窗 | 1 個覆蓋層 |
| 複雜度 | 高 | 低 |
| 可靠性 | 低（容易出錯） | 高 |
| 效能 | 差（多視窗） | 好（單視窗） |
| 維護性 | 差 | 好 |
| 與現有架構一致性 | 差 | 好 |

## 🎯 實作優勢

1. **簡單**：重用現有覆蓋層架構
2. **可靠**：與 FastWindowSwitcher 相同的方法
3. **一致**：與全螢幕覆蓋層模式一致
4. **高效**：只有一個視窗，繪製集中

## 🚀 下一步

### 立即行動

1. **刪除舊實作**：
   ```bash
   rm src/label_overlay.rs
   rm src/label_renderer.rs
   ```

2. **修改 main.rs**：
   ```rust
   // 移除 label_overlay 和 label_renderer 的 import
   // 移除 label_overlay_manager
   ```

3. **修改 overlay_renderer.rs**：
   ```rust
   // 新增 render_labels_only() 方法
   ```

4. **修改 overlay_manager.rs**：
   ```rust
   // 新增 show_labels_only() 方法
   ```

5. **測試**：
   ```bash
   cargo build --release
   .\target\release\window-selector.exe --debug
   ```

## ✅ 預期結果

**Win+Y 啟動標籤模式**：
```
1. 全螢幕透明覆蓋層出現
2. 每個視窗標題列上方顯示標籤
3. 標籤位置準確
4. 按字母直接跳轉到對應視窗
5. 按 Esc 關閉
```

**視覺效果**：
```
┌────────────────────────────────────┐
│  全螢幕透明覆蓋層                  │
│                                    │
│      ┌───┐                         │
│      │ A │  ← 標籤                 │
│      └───┘                         │
│  ┌─────────────┐                   │
│  │ 視窗 1      │                   │
│  │             │                   │
│  └─────────────┘                   │
│                                    │
│              ┌───┐                 │
│              │ S │  ← 標籤         │
│              └───┘                 │
│          ┌─────────────┐           │
│          │ 視窗 2      │           │
│          │             │           │
│          └─────────────┘           │
└────────────────────────────────────┘
```

---

**這是正確的實作方式！** 🎯

讓我知道是否要我立即開始重新實作？
