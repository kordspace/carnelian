#!/usr/bin/env bash
# browser-automation skill wrapper
# Category: automation
# Ported from THUMMIM: browser-automation-tool.ts

set -euo pipefail

# Parse input JSON
INPUT="${CARNELIAN_INPUT:-{}}"

# Extract fields using node -p for JSON parsing
ACTION=$(echo "$INPUT" | node -p "JSON.parse(require('fs').readFileSync(0, 'utf-8')).action || ''" 2>/dev/null || echo "")
PROFILE=$(echo "$INPUT" | node -p "JSON.parse(require('fs').readFileSync(0, 'utf-8')).profile || ''" 2>/dev/null || echo "")
TARGET_ID=$(echo "$INPUT" | node -p "JSON.parse(require('fs').readFileSync(0, 'utf-8')).targetId || ''" 2>/dev/null || echo "")
TARGET_URL=$(echo "$INPUT" | node -p "JSON.parse(require('fs').readFileSync(0, 'utf-8')).targetUrl || ''" 2>/dev/null || echo "")
REQUEST=$(echo "$INPUT" | node -p "JSON.stringify(JSON.parse(require('fs').readFileSync(0, 'utf-8')).request || {})" 2>/dev/null || echo "{}")

# Resolve browser control server URL
BROWSER_URL="${OPENCLAW_BROWSER_URL:-http://localhost:3000}"

# Validate action
if [ -z "$ACTION" ]; then
  echo '{"error": "Missing required field: action"}' >&2
  exit 1
fi

# Execute action
case "$ACTION" in
  status)
    curl -sf "$BROWSER_URL/" || exit 1
    ;;
  
  start)
    if [ -z "$PROFILE" ]; then
      echo '{"error": "Missing required field: profile"}' >&2
      exit 1
    fi
    curl -sf -X POST "$BROWSER_URL/start" \
      -H "Content-Type: application/json" \
      -d "{\"profile\": \"$PROFILE\"}" || exit 1
    ;;
  
  stop)
    if [ -z "$PROFILE" ]; then
      echo '{"error": "Missing required field: profile"}' >&2
      exit 1
    fi
    curl -sf -X POST "$BROWSER_URL/stop" \
      -H "Content-Type: application/json" \
      -d "{\"profile\": \"$PROFILE\"}" || exit 1
    ;;
  
  profiles)
    curl -sf "$BROWSER_URL/profiles" || exit 1
    ;;
  
  tabs)
    if [ -z "$PROFILE" ]; then
      echo '{"error": "Missing required field: profile"}' >&2
      exit 1
    fi
    curl -sf "$BROWSER_URL/tabs?profile=$PROFILE" || exit 1
    ;;
  
  open)
    if [ -z "$TARGET_URL" ]; then
      echo '{"error": "Missing required field: targetUrl"}' >&2
      exit 1
    fi
    curl -sf -X POST "$BROWSER_URL/tabs/open" \
      -H "Content-Type: application/json" \
      -d "{\"url\": \"$TARGET_URL\"}" || exit 1
    ;;
  
  focus)
    if [ -z "$TARGET_ID" ]; then
      echo '{"error": "Missing required field: targetId"}' >&2
      exit 1
    fi
    curl -sf -X POST "$BROWSER_URL/tabs/focus" \
      -H "Content-Type: application/json" \
      -d "{\"targetId\": \"$TARGET_ID\"}" || exit 1
    ;;
  
  close)
    if [ -z "$TARGET_ID" ]; then
      echo '{"error": "Missing required field: targetId"}' >&2
      exit 1
    fi
    curl -sf -X DELETE "$BROWSER_URL/tabs/$TARGET_ID" || exit 1
    ;;
  
  snapshot)
    if [ -z "$PROFILE" ]; then
      echo '{"error": "Missing required field: profile"}' >&2
      exit 1
    fi
    curl -sf "$BROWSER_URL/snapshot?format=ai&profile=$PROFILE" || exit 1
    ;;
  
  screenshot)
    if [ -z "$TARGET_ID" ]; then
      echo '{"error": "Missing required field: targetId"}' >&2
      exit 1
    fi
    curl -sf -X POST "$BROWSER_URL/screenshot" \
      -H "Content-Type: application/json" \
      -d "{\"targetId\": \"$TARGET_ID\"}" || exit 1
    ;;
  
  navigate)
    if [ -z "$TARGET_ID" ] || [ -z "$TARGET_URL" ]; then
      echo '{"error": "Missing required fields: targetId and targetUrl"}' >&2
      exit 1
    fi
    curl -sf -X POST "$BROWSER_URL/navigate" \
      -H "Content-Type: application/json" \
      -d "{\"targetId\": \"$TARGET_ID\", \"url\": \"$TARGET_URL\"}" || exit 1
    ;;
  
  console)
    if [ -z "$TARGET_ID" ]; then
      echo '{"error": "Missing required field: targetId"}' >&2
      exit 1
    fi
    curl -sf "$BROWSER_URL/console?targetId=$TARGET_ID" || exit 1
    ;;
  
  pdf)
    if [ -z "$TARGET_ID" ]; then
      echo '{"error": "Missing required field: targetId"}' >&2
      exit 1
    fi
    curl -sf -X POST "$BROWSER_URL/pdf" \
      -H "Content-Type: application/json" \
      -d "{\"targetId\": \"$TARGET_ID\"}" || exit 1
    ;;
  
  act)
    if [ "$REQUEST" = "{}" ]; then
      echo '{"error": "Missing required field: request"}' >&2
      exit 1
    fi
    curl -sf -X POST "$BROWSER_URL/act" \
      -H "Content-Type: application/json" \
      -d "$REQUEST" || exit 1
    ;;
  
  *)
    echo "{\"error\": \"Unknown action: $ACTION\"}" >&2
    exit 1
    ;;
esac
