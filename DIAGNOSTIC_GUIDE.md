# Terminal Emulator 黑屏问题诊断指南

## 问题描述

Terminal Emulator 启动后显示黑屏，看不到任何输入或输出。

## 诊断步骤

### 1. 运行诊断脚本

```bash
chmod +x run_diagnostic.sh
./run_diagnostic.sh
```

或手动运行：

```bash
cargo build --release
./target/release/terminal_emulator 2>&1 | tee output.log
```

### 2. 查找关键日志

运行后，在终端中输入命令（如 `echo hello`），然后按 Enter。查看输出中的以下日志：

**预期看到的日志行**:

```
✓ Shell session started successfully        # Shell 启动成功
[IOLoop] 后台 I/O 线程启动                   # I/O 线程已启动
[IOLoop] 输入已写入 PTY (5 字节)             # 你的命令被写入 PTY
[IOLoop] 读取 XX 字节的 PTY 数据             # Shell 的输出被读取
```

### 3. 如果看不到日志

#### 3.1 Shell 启动失败

**症状**: 看不到 "✓ Shell session started successfully"

**可能原因**:
- /bin/bash 不存在
- SHELL 环境变量指向不存在的路径
- PTY 初始化失败

**修复**:
```bash
# 检查 bash 是否存在
which bash
echo $SHELL

# 尝试手动指定 shell
SHELL=/bin/bash ./target/release/terminal_emulator
```

#### 3.2 I/O 线程没有启动

**症状**: 看不到任何 [IOLoop] 日志

**可能原因**:
- 线程创建失败
- Mutex 死锁
- Channel 初始化失败

**修复**:
```bash
# 检查系统资源
ps aux | wc -l              # 检查进程数
ulimit -u                   # 检查最大进程数

# 重新构建
cargo clean && cargo build --release
```

#### 3.3 输入没有被写入

**症状**: 看不到 "[IOLoop] 输入已写入" 日志

**可能原因**:
- 输入队列没有被收集
- channel 通信失败
- PTY write 失败

**修复**:
- 检查是否输入了任何内容
- 查看错误日志中的 "Write error"

#### 3.4 没有输出被读取

**症状**: 看不到 "[IOLoop] 读取" 日志

**可能原因**:
- Shell 没有输出任何内容
- PTY read 返回 0 或失败
- Shell 可能卡死或等待输入

**修复**:
```bash
# 使用简单的命令测试
# 输入: echo hello
# 然后: ls
# 然后: whoami
```

## 完整诊断流程

1. **第一步**: 查看是否有 "✓ Shell session started successfully"
   - 有 → 继续第二步
   - 没有 → Shell 启动失败，检查 SHELL 和 bash 路径

2. **第二步**: 查看是否有 "[IOLoop] 后台 I/O 线程启动"
   - 有 → 继续第三步
   - 没有 → 线程启动失败

3. **第三步**: 输入命令并按 Enter，查看是否有 "[IOLoop] 输入已写入 PTY"
   - 有 → 继续第四步
   - 没有 → 输入处理失败

4. **第四步**: 查看是否有 "[IOLoop] 读取" 日志
   - 有 → Shell 工作正常，问题可能在 UI 渲染
   - 没有 → PTY read 失败

## UI 渲染问题诊断

如果日志显示 Shell 和 I/O 正常工作，但屏幕仍然黑屏，问题可能在 UI 渲染：

```bash
# 添加更多日志来跟踪 UI 更新
# 编辑 main.rs 的 update() 函数，添加：
eprintln!("[UI] 事件: {:?}", event);
eprintln!("[UI] 终端网格: {} x {}", terminal.grid.len(), terminal.grid[0].len());
```

## 常见问题

### Q: 为什么没有初始提示符 ("$") 显示？

A: 初始化时写入了 "Terminal Emulator v0.4.0\r\nEnter your commands:\r\n$ " 到终端，但：
- 可能 UI 渲染有延迟
- 可能需要输入一个命令来触发渲染更新

### Q: 为什么输入命令后没有反应？

A: 这通常表示：
1. 命令没有被写入 PTY（检查日志）
2. Shell 没有输出（很少见，除非命令本身无输出）
3. 输出被读取但 UI 没有更新（渲染问题）

### Q: 如何禁用日志？

A: 编辑 shell.rs，移除或注释掉所有 `eprintln!` 行。

## 高级诊断

### 使用 strace 跟踪系统调用

```bash
strace -f -e trace=fork,execve,read,write,ioctl \
  ./target/release/terminal_emulator 2>&1 | grep -E "fork|execve|read|write"
```

### 检查 PTY 是否被正确创建

```bash
ls -la /dev/pts/          # 查看 PTY 设备
ps aux | grep bash        # 查看 shell 进程
```

### 手动测试 PTY

```rust
// 可以写一个简单的 Rust 程序来测试 PTY fork/exec
// 不通过 egui，直接与 PTY 交互
```

## 如果问题仍未解决

请收集以下信息：
1. 运行诊断脚本的完整输出
2. `cargo build --release` 的任何警告或错误
3. `echo $SHELL` 和 `which bash` 的输出
4. 操作系统版本 (`uname -a`)
5. 任何错误消息或异常行为

然后可以进一步调查根本原因。

---

**注**: 这个诊断指南假设 Terminal Emulator 可以成功运行（即使黑屏）。如果程序崩溃或无法启动，请检查编译错误和运行时错误。
