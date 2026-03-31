#!/bin/bash
# 诊断脚本：运行 Terminal Emulator 并显示所有输出

echo "================================================"
echo "Terminal Emulator 诊断脚本 v0.4.0"
echo "================================================"
echo ""
echo "系统信息:"
echo "  Shell: $SHELL"
echo "  Bash: $(which bash)"
echo "  uname: $(uname -a)"
echo ""

echo "构建状态:"
cargo build --release 2>&1 | tail -3
echo ""

echo "启动 Terminal Emulator (带完整日志):"
echo "  日志将显示在下方。输出 'exit' 命令来退出 shell。"
echo "  查看 stderr（以 [IOLoop] 开头的行）来诊断问题。"
echo ""
echo "================================================"

# 运行终端仿真器，将所有输出（包括 stderr）都显示出来
./target/release/terminal_emulator 2>&1 | tee terminal_output.log

echo ""
echo "================================================"
echo "运行完成。输出已保存到 terminal_output.log"
echo ""
echo "诊断要点："
echo "1. 查找 '[IOLoop]' 开头的行，这表示后台线程的活动"
echo "2. 查找 '✓ Shell session started successfully' 表示 shell 启动成功"
echo "3. 查找 '读取' 表示 PTY 数据正在被读取"
echo "4. 如果没有看到 '[IOLoop]' 行，说明后台线程没有启动"
echo ""
