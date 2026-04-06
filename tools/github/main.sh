#!/usr/bin/env bash
# GitHub API tool - requires GITHUB_TOKEN env var

set -e

# Read JSON input from stdin
input=$(cat)

# Extract arguments
endpoint=$(echo "$input" | jq -r '.arguments.endpoint // "user"')
owner=$(echo "$input" | jq -r '.arguments.owner // empty')
repo=$(echo "$input" | jq -r '.arguments.repo // empty')

# Validate GITHUB_TOKEN is set
if [ -z "$GITHUB_TOKEN" ]; then
    echo '{"error": "Missing required environment variable: GITHUB_TOKEN"}' >&2
    exit 1
fi

# Build API endpoint
case "$endpoint" in
    user)
        api_url="https://api.github.com/user"
        ;;
    repo|repository)
        if [ -z "$owner" ] || [ -z "$repo" ]; then
            echo '{"error": "Owner and repo required for repository endpoint"}' >&2
            exit 1
        fi
        api_url="https://api.github.com/repos/$owner/$repo"
        ;;
    *)
        api_url="https://api.github.com/$endpoint"
        ;;
esac

# Make API request with authentication
response=$(curl -s -w "\n%{http_code}" -H "Authorization: Bearer $GITHUB_TOKEN" \
    -H "Accept: application/vnd.github+json" \
    "$api_url")

# Extract status code (last line) and body
status_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | sed '$d')

if [ "$status_code" -ge 200 ] && [ "$status_code" -lt 300 ]; then
    echo "$body"
else
    echo "{\"error\": \"API request failed with status $status_code\"}" >&2
    exit 1
fi
