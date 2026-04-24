#!/usr/bin/env bash
# Weather lookup tool - demonstrates bearer_token auth strategy.
#
# This tool uses the bearer_token auth strategy. Configure via .auth.json:
#   {
#     "strategy": "bearer_token",
#     "required_env_vars": ["WEATHER_API_KEY"]
#   }
#
# The WEATHER_API_KEY env var will be injected as an environment variable
# when the tool runs. The tool then reads it and adds it as a header.
#
# Usage:
#   weather {city: "London", unit: "metric"}
#   weather {city: "Tokyo", unit: "imperial"}
#
# Arguments:
#   city   - City name (required)
#   unit   - Temperature unit: "metric" (default) or "imperial"
#   days   - Forecast days (1-7, default: 1)

set -euo pipefail

input=$(cat)
city=$(echo "$input" | jq -r '.arguments.city // empty')
unit=$(echo "$input" | jq -r '.arguments.unit // "metric"')
days=$(echo "$input" | jq -r '.arguments.days // 1')

if [ -z "$city" ]; then
    echo '{"error": "city is required"}' >&2
    exit 1
fi

# The API key comes from the auth strategy (injected as WEATHER_API_KEY env var)
if [ -z "${WEATHER_API_KEY:-}" ]; then
    echo '{"error": "WEATHER_API_KEY environment variable is not set. Configure bearer_token auth in .auth.json."}' >&2
    exit 1
fi

# Use wttr.in (free, no signup required) as the weather API
# Returns JSON when Accept: application/json is set
response=$(curl -s \
    -H "Accept: application/json" \
    "https://wttr.in/${city}?format=j1&u=${unit}&cnt=${days}")

# Validate response
if ! echo "$response" | jq empty 2>/dev/null; then
    echo '{"error": "Invalid response from weather API"}' >&2
    exit 1
fi

# Extract key fields for a cleaner output
jq '{
    city: .current_condition[0].nearest_area[0].areaName[0].value,
    temp_c: .current_condition[0].temp_C,
    temp_f: .current_condition[0].temp_F,
    humidity: .current_condition[0].humidity,
    wind_kph: .current_condition[0].windspeedKmph,
    condition: .current_condition[0].weatherDesc[0].value,
    is_forecast: (.forecast | length > 0)
}' <<< "$response"
