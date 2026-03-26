# 標籤模式重新實作計畫

## 🔍 FastWindowSwitcher 分析結果

### 關鍵發現

1. **不是獨立標籤視窗**：FastWindowSwitcher 使用**單一全螢幕透明覆蓋層**，在上面繪製所有標籤
2. **標籤位置計算**：
   - 使用 `SM_CXSIZEFRAME` 獲取視窗邊框寬度
   - 標籤放在視窗標題列上方（`window_rect.top() + windowBorderWidth`）
   - 考慮最大化視窗的特殊處理
3. **可見性檢查**：使用 `IsRectVisibleToUser` 確保標籤可見
4. **鍵盤處理**：Qt 的 `keyPressEvent`，不是 Windows keyboard hook

### 我們當前實作的問題

1. ❌ 為每個視窗創建獨立的標籤視窗 → 複雜且容易出錯
2. ❌ 標籤位置計算不準確
3. ❌ 沒有檢查標籤是否可見
4. ✅ 鍵盤處理正確（使用 keyboard hook）

## 🎯 新實作方案

### 方案：使用單一全螢幕覆蓋層

**優點**：
- 簡單可靠（類似現有的覆蓋層）
- 只需要一個視窗
- 繪製邏輯集中
- 與現有架構一致

**實作步驟**：

1. **重用現有的覆蓋層視窗**
   - 不創建新的標籤視窗
   - 在現有覆蓋層上繪製標籤

2. **新增標籤繪製模式**
   - 在 `overlay_renderer.rs` 新增 `render_labels_only()` 方法
   - 只繪製標籤，不繪製縮圖

3. **改進標籤位置計算**
   - 使用 `GetSystemMetrics(SM_CXSIZEFRAME)` 獲取邊框寬度
   - 正確計算標題列位置
   - 處理最大化視窗

4. **可見性檢查**
   - 確保標籤在螢幕可見區域內
   - 處理部分遮擋的視窗

## 📋 詳細實作

### 1. 修改 OverlayState

```rust
pub enum OverlayState {
    Hidden,
    FadingIn,
    Active { selected: Option<usize> },
    FadingOut { switch_target: Option<HWND> },
    LabelMode { selected: Option<usize> },  // 保持不變
}
```

### 2. 修改 overlay_renderer.rs

新增方法：
```rust
pub fn render_labels_only(
    &self,
    windows: &[WindowInfo],
    selected: Option<usize>,
) -> windows::core::Result<()> {
    // 1. 清除背景為透明
    // 2. 為每個視窗繪製標籤在標題列上方
    // 3. 選中的標籤使用強調色 + 光暈
}
```

### 3. 標籤位置計算

```rust
fn calculate_label_position(window_rect: RECT, is_maximized: bool) -> (i32, i32) {
    let border_width = unsafe {
        GetSystemMetrics(SM_CXSIZEFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER)
    };
    
    let label_x = window_rect.left + (window_rect.right - window_rect.left) / 2 - LABEL_WIDTH / 2;
    let label_y = if is_maximized {
        // 最大化視窗：標籤在可見區域頂部
        window_rect.top
    } else {
        // 一般視窗：標籤在標題列上方
        window_rect.top + border_width
    };
    
    (label_x, label_y)
}
```

### 4. 簡化流程

**啟動標籤模式**：
```rust
unsafe fn activate_label_mode(app: &mut AppState) {
    // 1. 獲取視窗列表
    // 2. 分配字母
    // 3. 顯示覆蓋層（透明背景 + 標籤）
    // 4. 啟動鍵盤 hook
    
    app.overlay_state = OverlayState::LabelMode { selected: None };
    app.overlay_manager.show_labels_only(&app.window_snapshot);
}
```

**按鍵處理**：
```rust
// 在 interaction.rs 中已經正確實作
// 標籤模式下按字母直接跳轉
if matches!(state, OverlayState::LabelMode { .. }) {
    if let Some(window) = windows.get(idx) {
        return KeyAction::SwitchTo(window.hwnd);
    }
}
```

## 🔧 實作優先級

### Phase 1: 核心功能（必須）
1. ✅ 修改 overlay_renderer.rs 支援標籤繪製
2. ✅ 改進標籤位置計算
3. ✅ 整合到現有覆蓋層

### Phase 2: 優化（可選）
1. ⏸️ 可見性檢查
2. ⏸️ 最大化視窗特殊處理
3. ⏸️ 多標籤位置（視窗很寬時）

## 🎯 預期效果

**啟動標籤模式（Win+Y）**：
```
1. 全螢幕透明覆蓋層出現
2. 每個視窗標題列上方顯示標籤
3. 標籤位置準確（考慮邊框）
4. 按字母直接跳轉
```

**視覺效果**：
```
        ┌───┐
        │ A │  ← 標籤（在標題列上方）
        └───┘
┌─────────────────────┐
│ ≡ 視窗標題列        │
├─────────────────────┤
│                     │
│   視窗內容          │
│                     │
└─────────────────────┘
```

## 🚀 下一步

1. **立即實作**：修改 overlay_renderer.rs 支援標籤模式
2. **測試**：確保標籤位置正確
3. **優化**：根據測試結果調整

---

**關鍵洞察**：不要為每個視窗創建獨立視窗，使用單一覆蓋層！
