#!/bin/bash
# Image info tool - demonstrates multiple content output types (text + image).
#
# This tool shows how to return both text and image content from a single
# tool call. mcp-cli supports returning a content array with mixed types.
#
# Usage:
#   image-info {path: "/path/to/image.png"}
#   image-info {path: "/path/to/image.png", action: "info"}
#
# Arguments:
#   path   - Path to image file (required)
#   action - "info" (default) or "base64"

set -euo pipefail

input=$(cat)
path=$(echo "$input" | jq -r '.arguments.path // empty')
action=$(echo "$input" | jq -r '.arguments.action // "info"')

if [ -z "$path" ]; then
    echo '{"error": "path is required"}' >&2
    exit 1
fi

if [ ! -f "$path" ]; then
    echo "{\"error\": \"File not found: $path\"}" >&2
    exit 1
fi

# Detect MIME type from extension
ext="${path##*.}"
ext_lower=$(echo "$ext" | tr '[:upper:]' '[:lower:]')
mime=""
case "$ext_lower" in
    png)  mime="image/png" ;;
    jpg|jpeg) mime="image/jpeg" ;;
    gif)  mime="image/gif" ;;
    webp) mime="image/webp" ;;
    svg)  mime="image/svg+xml" ;;
    *)    mime="application/octet-stream" ;;
esac

# Get file metadata
file_size=$(stat -c%s "$path" 2>/dev/null || stat -f%z "$path" 2>/dev/null || echo "unknown")
file_dims=$(identify -format "%wx%h" "$path" 2>/dev/null || echo "unknown")

case "$action" in
    info)
        # Return text content with file metadata
        jq -n \
            --arg name "$(basename "$path")" \
            --arg mime "$mime" \
            --arg size "$file_size" \
            --arg dims "$file_dims" \
            '{
                name: $name,
                mime_type: $mime,
                size_bytes: ($size | tonumber),
                dimensions: $dims,
                content_type: "text"
            }'
        ;;

    base64)
        # Return image content as base64
        base64_data=$(base64 -w0 "$path" 2>/dev/null || base64 "$path")
        jq -n \
            --arg data "$base64_data" \
            --arg mime "$mime" \
            --arg name "$(basename "$path")" \
            '{
                name: $name,
                mime_type: $mime,
                data: $data,
                content_type: "image"
            }'
        ;;

    *)
        echo "{\"error\": \"Unknown action: $action. Valid actions: info, base64\"}" >&2
        exit 1
        ;;
esac
