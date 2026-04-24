#!/bin/bash
# HTTP request tool - demonstrates complex argument parsing and flexible auth strategies.
#
# Usage patterns:
#   1. No auth:          curl-request {method: "GET", url: "https://api.example.com/data"}
#   2. Bearer token:     curl-request {method: "GET", url: "https://api.example.com/data", auth_type: "bearer", token: "xxx"}
#   3. API key:          curl-request {method: "GET", url: "https://api.example.com/data", auth_type: "api_key", api_key: "xxx"}
#   4. Custom headers:   curl-request {method: "POST", url: "...", headers: {"X-Custom": "val"}}
#   5. Request body:     curl-request {method: "POST", url: "...", body: {"key": "value"}}
#
# Auth config (.auth.json) can also be used:
#   {"strategy": "bearer_token", "required_env_vars": ["API_TOKEN"]}
# When configured, the API_TOKEN env var is injected automatically.

set -euo pipefail

# Read JSON input from stdin
input=$(cat)

# Extract arguments
method=$(echo "$input" | jq -r '.arguments.method // "GET"')
url=$(echo "$input" | jq -r '.arguments.url // empty')
auth_type=$(echo "$input" | jq -r '.arguments.auth_type // "none"')
token=$(echo "$input" | jq -r '.arguments.token // empty')
api_key=$(echo "$input" | jq -r '.arguments.api_key // empty')
headers_json=$(echo "$input" | jq -c '.arguments.headers // {}')
body=$(echo "$input" | jq -c '.arguments.body // empty')

# Validate URL is provided
if [ -z "$url" ]; then
    echo '{"error": "URL is required"}' >&2
    exit 1
fi

# Build curl command
curl_args=("-s" "-w" "\n%{http_code}")
curl_args+=("-X" "$method")
curl_args+=("-H" "Accept: application/json")
curl_args+=("-H" "Content-Type: application/json")

# Add custom headers from arguments
for key in $(echo "$headers_json" | jq -r 'keys[]'); do
    val=$(echo "$headers_json" | jq -r ".[\"$key\"]")
    curl_args+=("-H" "$key: $val")
done

# Apply auth header
case "$auth_type" in
    bearer)
        if [ -n "$token" ]; then
            curl_args+=("-H" "Authorization: Bearer $token")
        else
            echo '{"error": "Bearer auth requires a token argument"}' >&2
            exit 1
        fi
        ;;
    api_key)
        if [ -n "$api_key" ]; then
            curl_args+=("-H" "X-API-Key: $api_key")
        else
            echo '{"error": "API key auth requires an api_key argument"}' >&2
            exit 1
        fi
        ;;
    none)
        # No auth header
        ;;
    *)
        echo "{\"error\": \"Unknown auth_type: $auth_type\"}" >&2
        exit 1
        ;;
esac

# Add body for POST/PUT/PATCH
if [[ "$method" == "POST" || "$method" == "PUT" || "$method" == "PATCH" ]]; then
    if [ -n "$body" ] && [ "$body" != "null" ]; then
        curl_args+=("-d" "$body")
    fi
fi

# Execute request
response=$(curl "${curl_args[@]}" "$url")

# Extract status code (last line) and body
status_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | sed '$d')

# Return structured response
if [[ "$status_code" =~ ^[23] ]]; then
    jq -n --arg body "$body" --arg status "$status_code" '{status: ($status | tonumber), data: $body}'
else
    echo "{\"error\": \"Request failed with status $status_code\", \"body\": $body}" >&2
    exit 1
fi
