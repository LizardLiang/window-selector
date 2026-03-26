# 問題排查總結

## 🎯 當前狀態

### 快捷鍵設定
- **全螢幕覆蓋層**: `Ctrl+Alt+Q`
- **標籤模式**: `Win+Y`

### 已知問題
1. ✅ Win+B 被系統佔用 → 已改為 Ctrl+Alt+Q
2. ✅ Win+Q 被其他軟體佔用 → 已改為 Ctrl+Alt+Q
3. ⚠️ Win+Y 標籤模式切換問題 → 需要診斷

## 🚀 快速開始

### 方法 1: 使用批次檔（推薦）

```batch
# 雙擊執行
run_debug.bat
```

這會以 debug 模式啟動程式，顯示所有日誌。

### 方法 2: 命令列

```bash
cd "C:\Users\tc_tseng\Documents\APPS\window-selector"

# 先關閉舊程式
taskkill /F /IM window-selector.exe

# 重新編譯
cargo build --release

# 執行 debug 模式
.\target\release\window-selector.exe --debug
```

## 📋 測試檢查清單

### ✅ 測試覆蓋層模式 (Ctrl+Alt+Q)

- [ ] 按 `Ctrl+Alt+Q` 能開啟覆蓋層
- [ ] 看到所有視窗的縮圖方格
- [ ] 按字母鍵能選擇視窗（高亮顯示）
- [ ] 按 Enter 能跳轉到選中的視窗
- [ ] 按 Esc 能取消

**如果失敗**: 查看 [DIAGNOSIS.md](DIAGNOSIS.md) 問題 A

### ⚠️ 測試標籤模式 (Win+Y)

- [ ] 按 `Win+Y` 能啟動標籤模式
- [ ] 看到視窗標題列上方的標籤
- [ ] 標籤顯示正確的字母（A, S, D, F...）
- [ ] 按字母鍵能直接跳轉到對應視窗
- [ ] 按 Esc 能取消

**如果失敗**: 查看 [DIAGNOSIS.md](DIAGNOSIS.md) 對應問題

## 🔍 診斷流程

### 步驟 1: 收集日誌

1. 以 debug 模式執行程式
2. 執行有問題的操作
3. 查看控制台輸出或日誌檔案

### 步驟 2: 分析日誌

查找關鍵訊息：

```bash
# 查看標籤模式啟動
type "%APPDATA%\window-selector\logs\*.log" | findstr "label mode"

# 查看標籤創建
type "%APPDATA%\window-selector\logs\*.log" | findstr "Creating label"

# 查看按鍵處理
type "%APPDATA%\window-selector\logs\*.log" | findstr "Key pressed"
```

### 步驟 3: 對照預期輸出

**正常的標籤模式日誌應該包含**:

```
INFO  Label mode hotkey received
INFO  Activating label mode: 5 windows
INFO  Creating labels for 5 windows
INFO  Creating label a for window HWND(0x...) (視窗標題)
INFO  Creating label s for window HWND(0x...) (視窗標題)
...
INFO  Label mode activated with 5 labels
```

**按鍵時應該看到**:

```
DEBUG Key pressed: vk=83 (0x53), label_mode=true, windows=5
DEBUG Key action: SwitchTo(HWND(0x...))
```

## 🐛 常見問題快速修復

### Q1: 快捷鍵沒反應

**檢查**: 程式是否執行？
```bash
tasklist | findstr window-selector
```

**解決**: 執行程式
```bash
.\target\release\window-selector.exe --debug
```

### Q2: 標籤沒有顯示

**檢查**: 視窗是否可見？
- 視窗不能最小化
- 視窗必須在螢幕上可見

**解決**: 還原所有最小化的視窗

### Q3: 按字母鍵沒反應

**檢查日誌**: 
```bash
type "%APPDATA%\window-selector\logs\*.log" | findstr "Key pressed"
```

**可能原因**:
- 鍵盤 hook 沒有啟動
- 按鍵被其他程式攔截

### Q4: 跳轉到錯誤的視窗

**檢查**: 字母分配是否正確？

查看日誌中的視窗列表：
```bash
type "%APPDATA%\window-selector\logs\*.log" | findstr "Window snapshot"
```

## 📊 診斷資訊收集

如果問題仍然存在，請收集以下資訊：

### 1. 系統資訊
```bash
# Windows 版本
winver

# 執行中的程式
tasklist > running_processes.txt
```

### 2. 日誌檔案
```bash
# 複製最新的日誌
copy "%APPDATA%\window-selector\logs\*.log" debug_logs.txt
```

### 3. 設定檔
```bash
# 複製設定檔
copy "%APPDATA%\window-selector\config.toml" config_backup.toml
```

### 4. 螢幕截圖
- 標籤模式啟動時的螢幕截圖
- 控制台日誌輸出的截圖

## 🔧 重置設定

如果問題持續，嘗試重置設定：

```bash
# 備份舊設定
copy "%APPDATA%\window-selector\config.toml" "%APPDATA%\window-selector\config.toml.backup"

# 刪除設定檔（程式會重新創建預設設定）
del "%APPDATA%\window-selector\config.toml"

# 重新執行程式
.\target\release\window-selector.exe --debug
```

## 📞 回報問題

如果以上方法都無法解決，請提供：

1. **環境資訊**:
   - Windows 版本
   - 螢幕數量
   - 開啟的視窗數量

2. **日誌檔案**:
   - `%APPDATA%\window-selector\logs\` 中的最新日誌

3. **重現步驟**:
   - 詳細描述如何重現問題

4. **預期 vs 實際行為**:
   - 你預期會發生什麼
   - 實際發生了什麼

## 🎯 臨時解決方案

如果標籤模式一直有問題，可以暫時只使用覆蓋層模式：

```
Ctrl+Alt+Q → 看到縮圖 → 按字母選擇 → Enter 跳轉
```

這個模式功能更完整且更穩定。

---

**相關文件**:
- [DIAGNOSIS.md](DIAGNOSIS.md) - 詳細診斷指南
- [TEST_GUIDE.md](TEST_GUIDE.md) - 測試指南
- [QUICK_REFERENCE.md](QUICK_REFERENCE.md) - 快速參考

**版本**: 0.1.0  
**最後更新**: 2026-03-26
