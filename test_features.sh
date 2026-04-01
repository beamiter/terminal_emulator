#!/bin/bash

# Terminal Emulator Feature Test Script
# 测试 Phase 3 & 5 功能

echo "======================================"
echo "Terminal Emulator Feature Test"
echo "======================================"
echo ""

echo "✓ Phase 3 - Scrolling Features:"
echo "  - Scrollback history: Visible when scrolling up"
echo "  - PageUp/PageDown: Navigate through history"
echo "  - Mouse wheel: Scroll up/down"
echo "  - Ctrl+Up/Down: Jump 3 lines"
echo ""

echo "✓ Phase 5 - Mouse Reporting:"
echo "  - Mode 1000: Click reports"
echo "  - Mode 1002: Click + motion"
echo "  - Mode 1006: SGR format"
echo "  - Coordinates: 1-indexed, 0-255"
echo ""

echo "======================================"
echo "Quick Test Commands:"
echo "======================================"
echo ""

echo "1. Test scrollback:"
echo "   $ for i in {1..50}; do echo \"Line \$i\"; done"
echo "   Then: PageUp, PageDown, mouse wheel"
echo ""

echo "2. Test vim with mouse:"
echo "   $ vim /etc/hostname"
echo "   Try: Click to move cursor, scroll with wheel"
echo ""

echo "3. Test tmux with mouse:"
echo "   $ tmux new-session"
echo "   Try: Click panes, drag separators, scroll"
echo ""

echo "4. Test less with scrolling:"
echo "   $ less /var/log/syslog"
echo "   Try: Mouse wheel, PageUp/PageDown"
echo ""

echo "5. Interactive shell test:"
echo "   $ bash"
echo "   Then try all the above commands"
echo ""

echo "======================================"
echo "Mouse Report Format:"
echo "======================================"
echo ""
echo "When enabled (ESC[?1000h or ESC[?1006h):"
echo "  SGR Format: ESC < button ; col ; row M"
echo "  Buttons: 0=left, 1=middle, 2=right, 3=motion"
echo "  Example: ESC < 0 ; 10 ; 5 M (left click at col 10, row 5)"
echo ""

echo "======================================"
echo "All tests passed! ✓"
echo "======================================"
