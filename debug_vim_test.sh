#!/bin/bash

# Run terminal with full debug output and save to file
echo "启动终端模拟器并捕获 vim 操作日志..."
echo ""

cargo run --release 2>&1 | tee /tmp/vim_debug.log &
PID=$!

sleep 2

# Try to send keyboard input via stdin-based approach
# This is more reliable than xdotool for non-GUI input
sleep 1
echo "Opening vim..."

# Send commands via echo to the terminal's stdin
# NOTE: This approach may not work well with egui-based apps
# But let's try to use xdotool if available

if command -v xdotool &> /dev/null; then
    echo "Using xdotool to automate vim test..."

    # Give window time to appear
    sleep 1

    # Find the terminal window
    WINDOW=$(xdotool search --name "Terminal Emulator" | head -1)
    if [ -n "$WINDOW" ]; then
        echo "Found window: $WINDOW"

        # Type vim command
        xdotool type "vim ~/.vimrc"
        sleep 0.2
        xdotool key Return
        sleep 1

        # Press j key to scroll down
        echo "Sending j keys to scroll..."
        for i in {1..15}; do
            xdotool key j
            sleep 0.1
        done

        sleep 1

        # Quit vim
        xdotool key Escape
        sleep 0.2
        xdotool type ":q!"
        sleep 0.2
        xdotool key Return

        sleep 1
    fi
else
    echo "xdotool not found. You need to manually:"
    echo "  1. Type: vim ~/.vimrc"
    echo "  2. Press j multiple times"
    echo "  3. Type :q! and press Enter"
fi

# Kill the terminal
sleep 2
kill $PID 2>/dev/null || true
wait $PID 2>/dev/null || true

echo ""
echo "===== DEBUG LOG ====="
echo "Saved to: /tmp/vim_debug.log"
echo ""
echo "=== INPUT HEX DATA (first 50 lines) ==="
grep "\[INPUT-HEX\]" /tmp/vim_debug.log | head -50

echo ""
echo "=== ANSI COMMANDS (first 100 lines) ==="
grep "\[ANSI" /tmp/vim_debug.log | head -100

echo ""
echo "=== PUT_CHAR CALLS (first 50 lines) ==="
grep "\[PUT_CHAR\]" /tmp/vim_debug.log | head -50

echo ""
echo "=== SCROLL COMMANDS ==="
grep "\[ANSI-[ST]\|SCROLL\]" /tmp/vim_debug.log
