# 診斷指南 - 標籤模式問題排查

## 🔧 最新修改

### 快捷鍵變更
- **全螢幕覆蓋層**: `Win+Q` → `Ctrl+Alt+Q` (避免衝突)
- **標籤模式**: `Win+Y` (保持不變)

### 新增診斷日誌
已在程式中添加詳細日誌，幫助診斷問題。

## 📋 診斷步驟

### 步驟 1: 關閉舊程式並重新編譯

```bash
# 1. 關閉所有 window-selector 實例
# 在工作管理員中結束 window-selector.exe

# 2. 重新編譯
cd "C:\Users\tc_tseng\Documents\APPS\window-selector"
cargo build --release

# 3. 以 debug 模式執行（會顯示日誌）
.\target\release\window-selector.exe --debug
```

### 步驟 2: 測試標籤模式並查看日誌

1. **開啟 3-5 個視窗**（確保可見，不要最小化）

2. **按 Win+Y 啟動標籤模式**

3. **查看控制台輸出**，應該會看到類似：
   ```
   INFO  Activating label mode: 5 windows
   INFO  Creating labels for 5 windows
   INFO  Creating label a for window HWND(0x...) (Google Chrome)
   INFO  Creating label s for window HWND(0x...) (記事本)
   INFO  Creating label d for window HWND(0x...) (檔案總管)
   ...
   INFO  Label mode activated with 5 labels
   ```

4. **按字母鍵**（例如 `s`），查看日誌：
   ```
   DEBUG Key pressed: vk=83 (0x53), label_mode=true, windows=5
   DEBUG Key action: SwitchTo(HWND(0x...))
   ```

5. **檢查是否跳轉**

### 步驟 3: 檢查日誌檔案

如果控制台沒有輸出，查看日誌檔案：

```
%APPDATA%\window-selector\logs\
```

打開最新的日誌檔案，搜尋：
- `"Activating label mode"` - 確認標籤模式啟動
- `"Creating label"` - 確認標籤創建
- `"Key pressed"` - 確認按鍵接收
- `"SwitchTo"` - 確認跳轉動作

## 🐛 常見問題診斷

### 問題 A: 按 Win+Y 沒有反應

**檢查項目**:
1. 程式是否執行？（檢查系統匣）
2. 日誌中是否有 "Label mode hotkey received"？

**可能原因**:
- 快捷鍵被其他程式佔用
- 程式沒有正確註冊快捷鍵

**解決方法**:
```bash
# 查看日誌
type "%APPDATA%\window-selector\logs\*.log" | findstr "hotkey"

# 應該看到:
# INFO  Label hotkey registered: modifiers=0x4008 vk=0x59
```

### 問題 B: 標籤沒有顯示

**檢查項目**:
1. 視窗是否可見？（不能最小化）
2. 日誌中是否有 "Creating label"？
3. 日誌中是否有錯誤訊息？

**診斷**:
```bash
# 查看標籤創建日誌
type "%APPDATA%\window-selector\logs\*.log" | findstr "label"

# 應該看到:
# INFO  Creating labels for N windows
# INFO  Creating label a for window ...
# INFO  Creating label s for window ...
```

**可能原因**:
- 視窗被最小化（標籤只顯示在可見視窗上）
- 視窗沒有分配字母（超過 26 個視窗）
- 標籤視窗創建失敗

### 問題 C: 標籤顯示但按鍵沒反應

**檢查項目**:
1. 日誌中是否有 "Key pressed"？
2. 日誌中是否有 "Key action: SwitchTo"？

**診斷**:
```bash
# 按 Win+Y 後，按字母鍵（例如 s），查看日誌
type "%APPDATA%\window-selector\logs\*.log" | findstr "Key"

# 應該看到:
# DEBUG Key pressed: vk=83 (0x53), label_mode=true, windows=5
# DEBUG Key action: SwitchTo(HWND(...))
```

**可能原因**:
- 鍵盤 hook 沒有啟動
- 按鍵被其他程式攔截
- 字母與視窗不匹配

### 問題 D: 按鍵有反應但不跳轉

**檢查項目**:
1. 日誌中是否有 "SwitchTo"？
2. 日誌中是否有錯誤訊息？

**可能原因**:
- 視窗已關閉
- 視窗切換失敗（權限問題）

**診斷**:
```bash
# 查看視窗切換日誌
type "%APPDATA%\window-selector\logs\*.log" | findstr "switch"
```

## 🔍 詳細診斷資訊

### 查看視窗列表

在日誌中搜尋 "Window snapshot"，應該看到：
```
DEBUG Window snapshot: 5 windows
DEBUG   HWND(0x...) letter=Some('a') tag=None minimized=false title="Google Chrome"
DEBUG   HWND(0x...) letter=Some('s') tag=None minimized=false title="記事本"
DEBUG   HWND(0x...) letter=Some('d') tag=None minimized=false title="檔案總管"
...
```

### 查看標籤創建

在日誌中搜尋 "Creating label"，應該看到：
```
INFO  Creating label a for window HWND(0x...) (Google Chrome)
INFO  Creating label s for window HWND(0x...) (記事本)
INFO  Creating label d for window HWND(0x...) (檔案總管)
```

### 查看按鍵處理

在日誌中搜尋 "Key pressed"，應該看到：
```
DEBUG Key pressed: vk=83 (0x53), label_mode=true, windows=5
DEBUG Key action: SwitchTo(HWND(0x...))
```

## 📊 VK 碼對照表

| 按鍵 | VK 碼 (十進位) | VK 碼 (十六進位) |
|------|----------------|------------------|
| A    | 65             | 0x41             |
| S    | 83             | 0x53             |
| D    | 68             | 0x44             |
| F    | 70             | 0x46             |
| G    | 71             | 0x47             |
| H    | 72             | 0x48             |
| J    | 74             | 0x4A             |
| K    | 75             | 0x4B             |

## 🧪 測試腳本

創建一個測試批次檔 `test_label_mode.bat`:

```batch
@echo off
echo 測試標籤模式
echo.
echo 1. 確保程式以 debug 模式執行
echo 2. 開啟 3-5 個視窗
echo 3. 按 Win+Y
echo 4. 按字母鍵（例如 s）
echo.
echo 查看日誌:
type "%APPDATA%\window-selector\logs\*.log" | findstr /C:"label mode" /C:"Creating label" /C:"Key pressed"
pause
```

## 📝 回報格式

如果問題仍然存在，請提供以下資訊：

### 1. 環境資訊
- Windows 版本: ___________
- 開啟的視窗數量: ___________
- 視窗是否可見: ___________

### 2. 日誌片段

```
# 貼上日誌中的相關部分
# 包含:
# - "Activating label mode" 附近的日誌
# - "Creating label" 附近的日誌
# - "Key pressed" 附近的日誌
```

### 3. 觀察到的行為

- 按 Win+Y 後發生什麼？
- 是否看到標籤？
- 標籤位置是否正確？
- 按字母鍵後發生什麼？

### 4. 預期行為

- 應該跳轉到哪個視窗？
- 實際跳轉到哪個視窗（或沒有跳轉）？

## 🔧 臨時解決方案

如果標籤模式一直有問題，可以暫時只使用覆蓋層模式：

```
Ctrl+Alt+Q → 全螢幕覆蓋層 → 按字母選擇 → Enter 跳轉
```

這個模式應該是穩定的。

---

**版本**: 0.1.0  
**最後更新**: 2026-03-26
