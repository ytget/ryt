#!/bin/bash

# RYT Demo Script for asciinema
# This script demonstrates RYT's capabilities

echo "🚀 RYT - Rust Video Downloader Demo"
echo "=================================="
echo ""
echo "This demo shows RYT downloading a video from YouTube"
echo ""

# Show current directory and files
echo "📁 Current directory:"
pwd
echo ""

# Show RYT help
echo "❓ RYT help:"
./target/release/ryt --help
echo ""

# Build the application
echo "🔨 Building RYT..."
make build
echo ""

# Download a short video
echo "📹 Downloading video from YouTube..."
echo "URL: https://www.youtube.com/watch?v=dQw4w9WgXcQ"
echo ""

make download URL="https://www.youtube.com/watch?v=dQw4w9WgXcQ" OUTPUT="demo_video"

echo ""
echo "✅ Demo completed!"
echo ""

# Show downloaded file
echo "📁 Downloaded file:"
ls -la demo_video*
echo ""

# Show file info
echo "📊 File information:"
file demo_video*
echo ""

# Clean up
echo "🧹 Cleaning up..."
rm -f demo_video*
echo "Demo files removed."
echo ""
echo "🎉 Thank you for watching RYT demo!"
