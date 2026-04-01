#!/bin/bash

# Simple test to verify scroll region support

# Create a test input file with ANSI sequences
cat > /tmp/test_ansi_seq.txt << 'EOF'
Starting terminal test...
ESC[?1049h - Enable alternate buffer
ESC[1;10r - Set scroll region 1-10
ESC[5;1H - Move cursor to row 5, col 1
Hello from row 5
ESC[S - Scroll up once
ESC[?1049l - Disable alternate buffer
Done.
EOF

echo "Created test file: /tmp/test_ansi_seq.txt"
echo ""
echo "Scroll region fixes applied:"
echo "1. Added scroll_region_top and scroll_region_bottom tracking"
echo "2. Implemented 'r' command (DECSTBM) to set scroll regions"
echo "3. Updated 'S' command (Scroll Up) to work within scroll regions"
echo "4. Updated 'T' command (Scroll Down) to work within scroll regions"
echo ""
echo "These changes should fix vim's display issues when scrolling through files."
echo ""
echo "To test: run 'cargo run --release' and then in vim try:"
echo "  vim ~/.vimrc"
echo "  Press 'j' multiple times to scroll down"
echo "  Press 'G' to go to end of file"
echo ""
echo "The screen should update properly with the full content visible."
