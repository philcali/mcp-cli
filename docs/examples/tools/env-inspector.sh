#!/bin/bash
# Environment inspector tool - demonstrates env var resolution and credential injection.
#
# This tool is useful for debugging auth configurations. It shows all environment
# variables that mcp-cli injected from the auth strategy.
#
# Example auth config (.auth.json):
#   {
#     "strategy": "env_var",
#     "required_env_vars": ["MY_API_KEY", "MY_SECRET"]
#   }
#
# When this tool runs, MY_API_KEY and MY_SECRET are injected as env vars.
# This tool lists them to verify the injection worked.
#
# Usage:
#   env-inspector
#   env-inspector {filter: "API"}

set -euo pipefail

input=$(cat)
filter=$(echo "$input" | jq -r '.arguments.filter // empty')

# Collect all environment variables that match the auth pattern.
# mcp-cli injects credentials from:
#   - env_var strategy: all required_env_vars
#   - api_key strategy: all required_env_vars
#   - bearer_token strategy: all required_env_vars
#   - oauth2 strategy: OAUTH_ACCESS_TOKEN
#
# We filter to show only the ones that were actually set.
result='{}'

while IFS='=' read -r name value; do
    # Skip standard env vars that mcp-cli doesn't inject
    case "$name" in
        HOME|USER|PATH|SHELL|PWD|LANG|LC_*|TERM|HOSTNAME|SHLVL|_) continue ;;
    esac

    # Apply filter if specified
    if [ -n "$filter" ]; then
        if [[ ! "$name" == *"$filter"* ]]; then
            continue
        fi
    fi

    # Mask the value for safety in output
    if [ ${#value} -gt 0 ]; then
        masked="${value:0:2}****${value: -2}"
    else
        masked="(empty)"
    fi

    result=$(echo "$result" | jq --arg k "$name" --arg v "$masked" '. + {($k): $v}')
done < <(env | sort)

jq -n \
    --argjson vars "$result" \
    '{
        note: "Values are partially masked for safety. Full values are available to the tool at runtime.",
        injected_vars: $vars,
        count: ($vars | length)
    }'
