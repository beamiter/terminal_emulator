#!/bin/bash

# Test script to diagnose vim scrolling issue
# This will be run inside the terminal emulator

# Open vim with .vimrc
echo "Opening vim ~/.vimrc..."
vim ~/.vimrc << 'EOF'
# Send j key 5 times
jjjjj
# Wait a moment
# Send q to quit (with colon)
:q
EOF
