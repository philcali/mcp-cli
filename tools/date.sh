#!/bin/bash
# Date tool - returns current date/time with optional formatting
set -e

# Read JSON from stdin
input=$(cat)

# Extract format argument if provided (default: ISO 8601)
format=$(echo "$input" | jq -r '.arguments.format // "%Y-%m-%d %H:%M:%S"')

date +"$format"
