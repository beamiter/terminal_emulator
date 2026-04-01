#!/bin/bash

# Simpler test: just output what vim sends when scrolling
# We'll redirect stderr to capture debug logs

cd /home/mm/projects/terminal_emulator

echo "Starting vim with debug logging..."
echo ""

# Run cargo with redirected output
cargo run --release 2>&1 > /tmp/vim_complete.log &
APP_PID=$!

echo "Terminal started with PID $APP_PID"
sleep 2

# Send vim command via xdotool
WINDOW=$(xdotool search --name "Terminal Emulator" --class eframe 2>/dev/null | head -1)

if [ -z "$WINDOW" ]; then
    echo "Could not find Terminal Emulator window, trying alternative search..."
    WINDOW=$(xdotool search --name "Terminal" 2>/dev/null | head -1)
fi

if [ -z "$WINDOW" ]; then
    echo "ERROR: Could not find Terminal Emulator window"
    kill $APP_PID 2>/dev/null
    exit 1
fi

echo "Found window ID: $WINDOW"

# Type vim ~/.vimrc
xdotool type --window $WINDOW "vim ~/.vimrc"
sleep 0.3
xdotool key --window $WINDOW Return

sleep 3

echo "Sending 'j' key 10 times..."
for i in {1..10}; do
    echo "  Sending j #$i..."
    xdotool key --window $WINDOW j
    sleep 0.3
done

sleep 2

echo "Quitting vim..."
xdotool key --window $WINDOW Escape
sleep 0.3
xdotool type --window $WINDOW ":q!"
sleep 0.3
xdotool key --window $WINDOW Return

sleep 2

# Kill the app
kill $APP_PID 2>/dev/null
wait $APP_PID 2>/dev/null

echo ""
echo "Test complete!"
echo "Log file: /tmp/vim_complete.log"
echo ""
echo "=== Looking for ANSI sequences sent during vim scroll ==="
grep -n "\[ANSI.*j" /tmp/vim_complete.log || echo "No direct j handling found"
grep -n "\[ANSI\|INPUT-HEX\|PUT_CHAR\|SCROLL" /tmp/vim_complete.log | tail -200
