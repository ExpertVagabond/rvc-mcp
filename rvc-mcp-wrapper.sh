#!/bin/bash
# RVC WebUI MCP Server wrapper
# Sets environment variables and launches the MCP server
export RVC_URL="${RVC_URL:-http://localhost:7865}"
export RVC_DIR="/Volumes/Virtual Server/projects/ai-music-rvc"
export RVC_OUTPUT_DIR="$HOME/Desktop/AI-Music"
exec node "/Volumes/Virtual Server/projects/rvc-mcp/typescript/build/index.js"
