#!/bin/bash
# Microscope Memory - REST API Examples (curl)
API="http://localhost:6060/v1"

echo "=== Microscope Memory REST API ===="
echo ""

# 1. Status
echo "1. Engine status:"
curl -s $API/status | python3 -m json.tool
echo ""

# 2. Store a memory
echo "2. Store memory:"
curl -s -X POST $API/remember \n  -H "Content-Type: application/json" \n  -d '{"text":"Rust is a systems programming language","layer":"long_term","importance":8}'
echo ""

# 3. Recall
echo "3. Recall:"
curl -s "$API/recall?q=Rust&k=3" | python3 -m json.tool
echo ""

# 4. Store with emotion
echo "4. Store with emotion:"
curl -s -X POST $API/remember \n  -H "Content-Type: application/json" \n  -d '{"text":"I love this project","importance":10,"emotion_joy":0.9,"emotion_gratitude":0.8}'
echo ""

echo "=== Done ==="
