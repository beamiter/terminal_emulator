#!/bin/bash

# Run terminal emulator and log all output
cargo run --release 2>&1 | tee vim_test_output.log &
TERM_PID=$!

sleep 2

# Try to send vim startup and scroll commands via xdotool if available
if command -v xdotool &> /dev/null; then
    echo "Using xdotool to send commands"

    # Get window ID
    WINDOW_ID=$(xdotool search --name "Terminal Emulator" | head -1)

    if [ -z "$WINDOW_ID" ]; then
        echo "Could not find Terminal Emulator window"
    else
        echo "Found window ID: $WINDOW_ID"

        # Send vim ~/.vimrc command
        xdotool type "vim ~/.vimrc"
        xdotool key Return

        sleep 1

        # Send j key multiple times to scroll down
        for i in {1..10}; do
            xdotool key j
            sleep 0.1
        done

        sleep 1

        # Quit vim
        xdotool key Escape
        xdotool type ":q!"
        xdotool key Return
    fi
else
    echo "xdotool not available, will need manual testing"
fi

# Wait for terminal to exit
sleep 2
kill $TERM_PID 2>/dev/null || true
wait $TERM_PID 2>/dev/null || true

echo "Test complete. Check vim_test_output.log for details."
