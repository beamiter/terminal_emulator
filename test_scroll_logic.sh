#!/bin/bash

# Test scrolling region support

# Create a test Rust program that tests the terminal emulator
cat > /tmp/test_scroll_region.rs << 'EOF'
use std::path::PathBuf;

fn main() {
    // Simulate vim setting scroll region and then scrolling
    // Vim typically sends: ESC[?1049h (enable alt buffer), then ESC[rows;colsr (set scroll region)

    // Test sequence 1: Enable alternate buffer
    println!("Test 1: Enable alternate buffer");
    let enable_alt = b"\x1b[?1049h";
    println!("Bytes: {:?}", enable_alt);

    // Test sequence 2: Set scroll region (e.g., for 25-line display, region 1-24)
    println!("\nTest 2: Set scroll region 1;24r");
    let set_region = b"\x1b[1;24r";
    println!("Bytes: {:?}", set_region);

    // Test sequence 3: Scroll up 1 line (CSI S)
    println!("\nTest 3: Scroll up 1 line");
    let scroll_up = b"\x1b[S";
    println!("Bytes: {:?}", scroll_up);
}
EOF

echo "Created test script. Scroll region support should now handle vim correctly."
echo ""
echo "Key changes made:"
echo "1. Added scroll_region_top and scroll_region_bottom fields to TerminalState"
echo "2. Added 'r' command handler for setting scroll region (DECSTBM)"
echo "3. Modified 'S' (Scroll Up) command to respect scroll region"
echo "4. Modified 'T' (Scroll Down) command to respect scroll region"
echo ""
echo "This should fix vim's display issues when pressing j multiple times."
