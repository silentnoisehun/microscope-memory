#!/bin/bash
# Microscope Memory Demo Script

echo "🔬 Microscope Memory Demo"
echo "========================="
echo ""

# Build the index
echo "📦 Building index from memory layers..."
cargo run --release -- build

echo ""
echo "📊 Showing statistics..."
cargo run --release -- stats

echo ""
echo "🔍 Testing natural language recall..."
echo "Query: 'What is Ora?'"
cargo run --release -- recall "What is Ora?" 5

echo ""
echo "Query: 'memory system'"
cargo run --release -- recall "memory system" 5

echo ""
echo "🎯 Testing direct coordinate lookup..."
echo "Looking at center (0.25, 0.25, 0.25) at different zoom levels:"
for zoom in 0 1 2 3 4; do
    echo ""
    echo "Zoom $zoom:"
    cargo run --release -- look 0.25 0.25 0.25 $zoom 3
done

echo ""
echo "💾 Testing memory storage..."
cargo run --release -- store "This is a test memory from the demo script" long_term 7

echo ""
echo "🔍 Finding the test memory..."
cargo run --release -- find "demo script" 5

echo ""
echo "⚡ Running performance benchmark..."
cargo run --release -- bench

echo ""
echo "✅ Demo complete!"