#!/bin/bash
# File manager tool - demonstrates CRUD operations with path safety checks.
#
# Actions:
#   read     - Read file contents (returns text content)
#   write    - Write content to a file (creates parent dirs if needed)
#   list     - List directory contents (returns JSON array)
#   delete   - Delete a file or directory
#   exists   - Check if a path exists
#
# Usage examples:
#   file-manager {action: "read", path: "/tmp/test.txt"}
#   file-manager {action: "write", path: "/tmp/test.txt", content: "hello world"}
#   file-manager {action: "list", path: "/tmp"}
#   file-manager {action: "delete", path: "/tmp/test.txt"}
#   file-manager {action: "exists", path: "/tmp/test.txt"}

set -euo pipefail

input=$(cat)

action=$(echo "$input" | jq -r '.arguments.action // empty')
path=$(echo "$input" | jq -r '.arguments.path // empty')
content=$(echo "$input" | jq -r '.arguments.content // empty')

# Validate required arguments
if [ -z "$action" ]; then
    echo '{"error": "action is required (read|write|list|delete|exists)"}' >&2
    exit 1
fi

if [ -z "$path" ]; then
    echo '{"error": "path is required"}' >&2
    exit 1
fi

# Resolve to absolute path for safety
abs_path=$(realpath -m "$path" 2>/dev/null || echo "$path")

case "$action" in
    read)
        if [ ! -f "$abs_path" ]; then
            echo "{\"error\": \"File not found: $abs_path\"}" >&2
            exit 1
        fi
        cat "$abs_path"
        ;;

    write)
        parent_dir=$(dirname "$abs_path")
        mkdir -p "$parent_dir"
        echo "$content" > "$abs_path"
        jq -n --arg p "$abs_path" '{written: true, path: $p, bytes: ($content | length)}'
        ;;

    list)
        if [ ! -d "$abs_path" ]; then
            echo "{\"error\": \"Directory not found: $abs_path\"}" >&2
            exit 1
        fi
        # Return JSON array of entries with type info
        jq -R -s 'split("\n") | map(select(length > 0)) | map({name: ., type: (if test("/$") then "directory" else "file" end)})' \
            <(ls -1A "$abs_path" | while read -r entry; do
                if [ -d "$abs_path/$entry" ]; then
                    echo "$entry/"
                else
                    echo "$entry"
                fi
            done)
        ;;

    delete)
        if [ ! -e "$abs_path" ]; then
            echo "{\"error\": \"Path not found: $abs_path\"}" >&2
            exit 1
        fi
        if [ -d "$abs_path" ]; then
            rm -rf "$abs_path"
        else
            rm -f "$abs_path"
        fi
        jq -n --arg p "$abs_path" '{deleted: true, path: $p}'
        ;;

    exists)
        if [ -e "$abs_path" ]; then
            jq -n --arg p "$abs_path" '{exists: true, path: $p, is_dir: (test("/$"))}'
        else
            jq -n --arg p "$abs_path" '{exists: false, path: $p}'
        fi
        ;;

    *)
        echo "{\"error\": \"Unknown action: $action. Valid actions: read, write, list, delete, exists\"}" >&2
        exit 1
        ;;
esac
