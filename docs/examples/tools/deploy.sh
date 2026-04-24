#!/bin/bash
# Deployment tool - demonstrates oauth2 auth strategy (client credentials flow).
#
# This tool uses the oauth2 auth strategy. Configure via .auth.json:
#   {
#     "strategy": "oauth2",
#     "required_env_vars": ["DEPLOY_CLIENT_SECRET"],
#     "oauth_config": {
#       "client_id_env": "DEPLOY_CLIENT_ID",
#       "token_url": "https://auth.example.com/oauth2/token",
#       "scopes": ["deploy:write"]
#     }
#   }
#
# The OAuth2 flow:
#   1. mcp-cli reads DEPLOY_CLIENT_ID from env (from DEPLOY_CLIENT_ID env var)
#   2. mcp-cli reads DEPLOY_CLIENT_SECRET from env (from required_env_vars)
#   3. POSTs to token_url with grant_type=client_credentials
#   4. Caches the access token (shared across requests in daemon mode)
#   5. Injects OAUTH_ACCESS_TOKEN env var into the tool
#
# Usage:
#   deploy {action: "deploy", service: "api", version: "v1.2.3"}
#   deploy {action: "rollback", service: "api", version: "v1.2.2"}
#   deploy {action: "status", service: "api"}
#
# Arguments:
#   action   - "deploy", "rollback", or "status" (required)
#   service  - Service name (required)
#   version  - Version tag (required for deploy/rollback)

set -euo pipefail

input=$(cat)
action=$(echo "$input" | jq -r '.arguments.action // empty')
service=$(echo "$input" | jq -r '.arguments.service // empty')
version=$(echo "$input" | jq -r '.arguments.version // empty')

# Validate required arguments
if [ -z "$action" ]; then
    echo '{"error": "action is required (deploy|rollback|status)"}' >&2
    exit 1
fi

if [ -z "$service" ]; then
    echo '{"error": "service is required"}' >&2
    exit 1
fi

# OAuth2 access token is injected by mcp-cli auth system
if [ -z "${OAUTH_ACCESS_TOKEN:-}" ]; then
    echo '{"error": "OAuth2 token not available. Ensure oauth2 auth is configured in .auth.json."}' >&2
    exit 1
fi

case "$action" in
    deploy)
        if [ -z "$version" ]; then
            echo '{"error": "version is required for deploy action"}' >&2
            exit 1
        fi
        # In production, this would call a deployment API with the token.
        # Demonstrating the token is available:
        jq -n \
            --arg service "$service" \
            --arg version "$version" \
            --arg token_prefix "${OAUTH_ACCESS_TOKEN:0:8}" \
            '{
                action: "deploy",
                service: $service,
                version: $version,
                token_available: true,
                token_prefix: $token_prefix,
                note: "Replace with actual deployment API call using Authorization: Bearer $OAUTH_ACCESS_TOKEN"
            }'
        ;;

    rollback)
        if [ -z "$version" ]; then
            echo '{"error": "version is required for rollback action"}' >&2
            exit 1
        fi
        jq -n \
            --arg service "$service" \
            --arg version "$version" \
            '{action: "rollback", service: $service, target_version: $version}'
        ;;

    status)
        jq -n \
            --arg service "$service" \
            '{action: "status", service: $service, current_version: "v1.2.3", status: "running"}'
        ;;

    *)
        echo "{\"error\": \"Unknown action: $action. Valid actions: deploy, rollback, status\"}" >&2
        exit 1
        ;;
esac
