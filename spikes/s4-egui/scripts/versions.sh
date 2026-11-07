#!/bin/sh
# Versions quoted in the report: toolchain, crates, Mesa, GL and Vulkan
# devices on the current DISPLAY, and the fonts in the image.
set -u
rustc --version
cargo --version
cargo tree -e normal --depth 1 2>/dev/null | /usr/bin/grep -E "eframe|egui|wgpu|glow|tree-sitter|image" | sort -u
cargo tree -i egui@0.36.0 -e normal 2>/dev/null | head -3
echo "DISPLAY=$DISPLAY"
glxinfo -B 2>/dev/null | /usr/bin/grep -E "OpenGL (vendor|renderer|core profile version|version) string|Device:"
vulkaninfo --summary 2>/dev/null | /usr/bin/grep -E "deviceName|driverInfo|apiVersion|deviceType"
cat /opt/fonts/SOURCES.txt
