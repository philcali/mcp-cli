#!/bin/bash
# Database query tool - demonstrates api_key auth strategy.
#
# This tool uses the api_key auth strategy. Configure via .auth.json:
#   {
#     "strategy": "api_key",
#     "required_env_vars": ["DB_API_URL", "DB_API_KEY"]
#   }
#
# Both DB_API_URL and DB_API_KEY are injected as env vars.
# The tool sends DB_API_KEY as an Authorization header to DB_API_URL.
#
# Usage:
#   db-query {query: "SELECT * FROM users LIMIT 10", engine: "postgres"}
#   db-query {query: "INSERT INTO logs VALUES (?)", engine: "mysql"}
#
# Arguments:
#   query   - SQL query to execute (required)
#   engine  - Database engine: "postgres", "mysql", "sqlite" (default: "postgres")

set -euo pipefail

input=$(cat)
query=$(echo "$input" | jq -r '.arguments.query // empty')
engine=$(echo "$input" | jq -r '.arguments.engine // "postgres"')

if [ -z "$query" ]; then
    echo '{"error": "query is required"}' >&2
    exit 1
fi

# Validate engine
case "$engine" in
    postgres|mysql|sqlite) ;;
    *)
        echo "{\"error\": \"Unknown engine: $engine. Supported: postgres, mysql, sqlite\"}" >&2
        exit 1
        ;;
esac

# Check required env vars from auth config
if [ -z "${DB_API_URL:-}" ]; then
    echo '{"error": "DB_API_URL environment variable is not set. Configure api_key auth in .auth.json."}' >&2
    exit 1
fi

if [ -z "${DB_API_KEY:-}" ]; then
    echo '{"error": "DB_API_KEY environment variable is not set. Configure api_key auth in .auth.json."}' >&2
    exit 1
fi

# In a real implementation, this would connect to a database service.
# Here we demonstrate the pattern: env vars from auth config are available.
jq -n \
    --arg engine "$engine" \
    --arg query "$query" \
    --arg url "$DB_API_URL" \
    '{
        status: "simulated",
        engine: $engine,
        query: $query,
        api_url: $url,
        note: "This example uses an API endpoint. Replace with actual database driver for production use."
    }'
