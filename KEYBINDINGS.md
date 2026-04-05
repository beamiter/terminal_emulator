# 快捷键配置指南

## 配置文件位置

```
~/.config/terminal_emulator/keybindings.toml
```

## 配置文件格式

配置文件使用 TOML 格式。每行定义一个快捷键绑定：

```toml
[keybindings]
"快捷键字符串" = "命令"
```

### 快捷键字符串格式

快捷键字符串使用 `+` 分隔符连接修饰符和按键：

```
ctrl+shift+a       # Ctrl+Shift+A
alt+f              # Alt+F
ctrl+pageup        # Ctrl+Page Up
shift+f1           # Shift+F1
```

**修饰符**（按照字母顺序）：
- `ctrl` - Control 键
- `shift` - Shift 键
- `alt` - Alt 键
- `super` - Super/Windows 键

**按键名称**：
- 字母：`a`-`z`（小写）
- 数字：`0`-`9`
- 功能键：`f1`-`f12`
- 特殊键：`return`, `escape`, `backspace`, `tab`, `delete`, `insert`, `home`, `end`, `pageup`, `pagedown`, `up`, `down`, `left`, `right`

## 可用命令

### 会话管理

| 命令 | 说明 |
|------|------|
| `session:new` | 创建新会话 |
| `session:close` | 关闭当前会话 |
| `session:next` | 切换到下一个会话 |
| `session:prev` | 切换到上一个会话 |
| `session:jump:0` - `session:jump:8` | 跳转到第 N 个会话（0-8） |

### 编辑操作

| 命令 | 说明 |
|------|------|
| `edit:copy` | 复制选中文本到剪贴板 |
| `edit:paste` | 从剪贴板粘贴 |

### 搜索操作

| 命令 | 说明 |
|------|------|
| `search:open` | 打开/关闭搜索面板 |
| `search:close` | 关闭搜索面板 |
| `search:next` | 跳转到下一个匹配项 |
| `search:prev` | 跳转到上一个匹配项 |
| `search:history:prev` | 搜索历史上一条 |
| `search:history:next` | 搜索历史下一条 |

### 终端操作

| 命令 | 说明 |
|------|------|
| `terminal:send_sigint` | 发送 Ctrl+C（SIGINT） |
| `terminal:send_eof` | 发送 Ctrl+D（EOF） |
| `terminal:clear` | 清屏（Ctrl+L） |
| `terminal:scroll_up` | 向上滚动 |
| `terminal:scroll_down` | 向下滚动 |

### 窗口操作

| 命令 | 说明 |
|------|------|
| `window:close` | 关闭窗口 |

## 默认快捷键

```toml
[keybindings]
# 会话管理
"ctrl+shift+t" = "session:new"
"ctrl+w" = "session:close"
"ctrl+tab" = "session:next"
"ctrl+shift+tab" = "session:prev"
"ctrl+pagedown" = "session:next"
"ctrl+pageup" = "session:prev"

# 会话切换（数字）
"ctrl+0" = "session:jump:0"
"ctrl+1" = "session:jump:1"
"ctrl+2" = "session:jump:2"
"ctrl+3" = "session:jump:3"
"ctrl+4" = "session:jump:4"
"ctrl+5" = "session:jump:5"
"ctrl+6" = "session:jump:6"
"ctrl+7" = "session:jump:7"
"ctrl+8" = "session:jump:8"

# 编辑操作
"ctrl+shift+c" = "edit:copy"
"ctrl+shift+v" = "edit:paste"

# 搜索操作
"ctrl+shift+f" = "search:open"

# 终端操作
"ctrl+up" = "terminal:scroll_up"
"ctrl+down" = "terminal:scroll_down"
```

## 自定义示例

### 使用 Vim 风格快捷键

```toml
[keybindings]
# 用 Alt+J/K 替代 Ctrl+Tab/Shift+Tab 切换会话
"alt+j" = "session:next"
"alt+k" = "session:prev"

# 用 Alt+1-9 直接切换会话
"alt+1" = "session:jump:0"
"alt+2" = "session:jump:1"
"alt+3" = "session:jump:2"
"alt+4" = "session:jump:3"
"alt+5" = "session:jump:4"
"alt+6" = "session:jump:5"
"alt+7" = "session:jump:6"
"alt+8" = "session:jump:7"
"alt+9" = "session:jump:8"

# 保持其他默认快捷键
"ctrl+shift+t" = "session:new"
"ctrl+w" = "session:close"
"ctrl+shift+c" = "edit:copy"
"ctrl+shift+v" = "edit:paste"
"ctrl+shift+f" = "search:open"
```

### 使用 Emacs 风格快捷键

```toml
[keybindings]
# 用 Ctrl+N/P 替代 Ctrl+Tab/Shift+Tab 切换会话
"ctrl+n" = "session:next"
"ctrl+p" = "session:prev"

# 保持搜索和编辑快捷键
"ctrl+shift+c" = "edit:copy"
"ctrl+shift+v" = "edit:paste"
"ctrl+shift+f" = "search:open"
```

## 注意事项

1. **快捷键冲突**：如果一个快捷键同时映射到多个命令，后面的定义会覆盖前面的
2. **区分大小写**：配置文件会自动转换为小写，所以 `CTRL+A` 和 `ctrl+a` 是相同的
3. **保留快捷键**：某些快捷键（如 `Ctrl+C`）由终端应用本身处理，不能通过配置修改
4. **配置重载**：修改配置文件后需要重启应用才能生效

## 验证配置

配置文件加载失败时，应用会自动使用默认快捷键。检查日志或状态消息了解具体错误。
