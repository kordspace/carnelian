#!/usr/bin/env bash
# CARNELIAN: Import THUMMIM Skills
# Scaffolds skill directories from THUMMIM tool definitions

set -euo pipefail

# ─────────────────────────────────────────────
# Path Setup
# ─────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CARNELIAN_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
THUMMIM_TOOLS="$(cd "$CARNELIAN_ROOT/../THUMMIM/thummim/src/agents/tools" && pwd)"
REGISTRY_DIR="$CARNELIAN_ROOT/skills/registry"

# ─────────────────────────────────────────────
# Argument Parsing
# ─────────────────────────────────────────────
FORCE=0
DRY_RUN=0
ONLY_SKILL=""

for arg in "$@"; do
    case $arg in
        --force)
            FORCE=1
            ;;
        --dry-run)
            DRY_RUN=1
            ;;
        --skill)
            shift_next=true
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --force       Overwrite existing files"
            echo "  --dry-run     Print actions without executing"
            echo "  --skill NAME  Only scaffold the specified skill"
            echo "  --help        Show this help message"
            exit 0
            ;;
        *)
            if [ "${shift_next:-false}" = true ]; then
                ONLY_SKILL="$arg"
                shift_next=false
            fi
            ;;
    esac
done

echo "=== CARNELIAN: Import THUMMIM Skills ==="
echo ""

# ─────────────────────────────────────────────
# Helper: Extract Tool Name
# ─────────────────────────────────────────────
extract_tool_name() {
    local file="$1"
    local name
    
    # Try single-quoted variant first
    name=$(sed -n "s/.*name: '\([^']*\)'.*/\1/p" "$file" | head -1)
    
    # Fall back to double-quoted variant
    if [ -z "$name" ]; then
        name=$(sed -n 's/.*name: "\([^"]*\)".*/\1/p' "$file" | head -1)
    fi
    
    echo "$name"
}

# ─────────────────────────────────────────────
# Helper: Extract Description
# ─────────────────────────────────────────────
extract_description() {
    local file="$1"
    local desc
    
    desc=$(sed -n 's/.*description: "\([^"]*\)".*/\1/p' "$file" | head -1)
    
    echo "$desc"
}

# ─────────────────────────────────────────────
# Helper: Scaffold Skill
# ─────────────────────────────────────────────
scaffold_skill() {
    local skill_name="$1"
    local tool_file="$2"
    local category="$3"
    local runtime="$4"
    local emoji="$5"
    local env_vars="$6"
    local capabilities="$7"
    local network="$8"
    local fallback_desc="$9"
    
    # Skip if ONLY_SKILL is set and doesn't match
    if [ -n "$ONLY_SKILL" ] && [ "$ONLY_SKILL" != "$skill_name" ]; then
        return
    fi
    
    # Resolve tool path
    local tool_path="$THUMMIM_TOOLS/$tool_file"
    if [ ! -f "$tool_path" ]; then
        echo "⚠ Warning: $tool_file not found, skipping $skill_name"
        return
    fi
    
    # Extract metadata
    local tool_name
    tool_name=$(extract_tool_name "$tool_path")
    if [ -z "$tool_name" ]; then
        tool_name="$skill_name"
    fi
    
    local description
    description=$(extract_description "$tool_path")
    if [ -z "$description" ]; then
        description="$fallback_desc"
    fi
    
    # Set up directories
    local skill_dir="$REGISTRY_DIR/$skill_name"
    local scripts_dir="$skill_dir/scripts"
    
    # Dry-run mode
    if [ "$DRY_RUN" -eq 1 ]; then
        echo "[dry-run] would create $skill_dir/{SKILL.md,skill.json,scripts/index.js}"
        return
    fi
    
    # Create directories
    mkdir -p "$scripts_dir"
    
    # ─────────────────────────────────────────────
    # Generate SKILL.md
    # ─────────────────────────────────────────────
    local skill_md="$skill_dir/SKILL.md"
    if [ -f "$skill_md" ] && [ "$FORCE" -eq 0 ]; then
        echo "⊘ $skill_name (SKILL.md exists, skipping)"
    else
    
    # Build env vars YAML
    local env_yaml=""
    local primary_env=""
    if [ -n "$env_vars" ]; then
        for env_var in $env_vars; do
            if [ -z "$primary_env" ]; then
                primary_env="$env_var"
            fi
            env_yaml="${env_yaml}        - ${env_var}\n"
        done
    fi
    
    # Build capabilities YAML
    local cap_yaml=""
    if [ -n "$capabilities" ]; then
        for cap in $capabilities; do
            cap_yaml="${cap_yaml}      - ${cap}\n"
        done
    fi
    
    # Build sandbox env YAML
    local sandbox_env_yaml=""
    if [ -n "$env_vars" ]; then
        for env_var in $env_vars; do
            sandbox_env_yaml="${sandbox_env_yaml}        ${env_var}: \"\${${env_var}}\"\n"
        done
    fi
    
    cat > "$skill_md" <<EOF
---
name: ${skill_name}
description: "${description}"
metadata:
  openclaw:
    emoji: "${emoji}"
EOF

    if [ -n "$env_yaml" ]; then
        cat >> "$skill_md" <<EOF
    requires:
      env:
$(echo -e "$env_yaml" | sed 's/^//')
    primaryEnv: ${primary_env}
EOF
    fi

    cat >> "$skill_md" <<EOF
  carnelian:
    runtime: ${runtime}
    version: "0.1.0"
    sandbox:
      network: ${network}
      resourceLimits:
        maxMemoryMB: 512
        maxCpuPercent: 50
        timeoutSecs: 300
EOF

    if [ -n "$sandbox_env_yaml" ]; then
        cat >> "$skill_md" <<EOF
      env:
$(echo -e "$sandbox_env_yaml" | sed 's/^//')
EOF
    fi

    if [ -n "$cap_yaml" ]; then
        cat >> "$skill_md" <<EOF
    capabilities:
$(echo -e "$cap_yaml" | sed 's/^//')
EOF
    fi

    cat >> "$skill_md" <<EOF
---

# ${skill_name}

Ported from THUMMIM \`${tool_file}\` (THUMMIM tool name: \`${tool_name}\`).

## Input

<!-- TODO: Document input parameters from THUMMIM TypeBox schema -->

## Output

<!-- TODO: Document output format -->
EOF
    fi
    
    # ─────────────────────────────────────────────
    # Generate skill.json
    # ─────────────────────────────────────────────
    local skill_json="$skill_dir/skill.json"
    if [ -f "$skill_json" ] && [ "$FORCE" -eq 0 ]; then
        echo "⊘ $skill_name (skill.json exists, skipping)"
    else
    
    # Build capabilities JSON array
    local cap_json=""
    if [ -n "$capabilities" ]; then
        local first=1
        for cap in $capabilities; do
            if [ "$first" -eq 1 ]; then
                cap_json="\"${cap}\""
                first=0
            else
                cap_json="${cap_json}, \"${cap}\""
            fi
        done
    fi
    
    cat > "$skill_json" <<EOF
{
  "name": "${skill_name}",
  "description": "${description}",
  "runtime": "${runtime}",
  "version": "0.1.0",
  "capabilities_required": [${cap_json}],
  "sandbox": {
    "network": "${network}",
    "max_memory_mb": 512,
    "max_cpu_percent": 50
  },
  "openclaw_compat": {
    "emoji": "${emoji}",
    "tags": ["${category}"]
  }
}
EOF
    fi
    
    # ─────────────────────────────────────────────
    # Generate scripts/index.js
    # ─────────────────────────────────────────────
    local index_js="$scripts_dir/index.js"
    if [ -f "$index_js" ] && [ "$FORCE" -eq 0 ]; then
        echo "⊘ $skill_name (index.js exists, skipping)"
    else
    
    cat > "$index_js" <<EOF
/**
 * ${skill_name} skill wrapper
 * Category: ${category}
 * Ported from THUMMIM: ${tool_file} (tool: ${tool_name})
 *
 * Sandbox globals available: fetch, URL, URLSearchParams, process.env
 * Required env vars: ${env_vars}
 */

// module.exports.run receives the parsed input object and must return a result.
module.exports.run = async (input) => {
  // TODO: Implement ${skill_name}
  // Reference: THUMMIM/thummim/src/agents/tools/${tool_file}
  throw new Error("Not yet implemented: ${skill_name}");
};
EOF
    fi
    
    echo "✓ ${skill_name} (${tool_file})"
}

# ─────────────────────────────────────────────
# Skill Definitions (20 calls)
# ─────────────────────────────────────────────

scaffold_skill "web-search" "web-search.ts" "research" "node" "🔍" \
    "BRAVE_API_KEY" "net.http" "full" \
    "Search the web using Brave Search API"

scaffold_skill "web-fetch" "web-fetch.ts" "research" "node" "🌐" \
    "FIRECRAWL_API_KEY" "net.http" "full" \
    "Fetch and parse web content using Firecrawl"

scaffold_skill "discord-send" "discord-actions.ts" "communication" "node" "💬" \
    "DISCORD_BOT_TOKEN" "net.http" "full" \
    "Send messages and manage Discord channels"

scaffold_skill "telegram-send" "telegram-actions.ts" "communication" "node" "✈️" \
    "TELEGRAM_BOT_TOKEN" "net.http" "full" \
    "Send messages via Telegram bot"

scaffold_skill "slack-send" "slack-actions.ts" "communication" "node" "💼" \
    "SLACK_BOT_TOKEN" "net.http" "full" \
    "Send messages and manage Slack channels"

scaffold_skill "whatsapp-send" "whatsapp-actions.ts" "communication" "node" "📱" \
    "" "net.http" "full" \
    "Send messages via WhatsApp"

scaffold_skill "message-send" "message-tool.ts" "communication" "node" "💌" \
    "" "net.http" "localhost" \
    "Send messages to other agents or sessions"

scaffold_skill "image-generate" "image-tool.ts" "creative" "node" "🖼️" \
    "OPENAI_API_KEY" "net.http" "full" \
    "Generate images using DALL-E"

scaffold_skill "image-analyze" "image-tool.ts" "creative" "node" "🔬" \
    "OPENAI_API_KEY" "net.http" "full" \
    "Analyze images using vision models"

scaffold_skill "text-to-speech" "tts-tool.ts" "creative" "node" "🔊" \
    "OPENAI_API_KEY" "net.http" "full" \
    "Convert text to speech using OpenAI TTS"

scaffold_skill "canvas-render" "canvas-tool.ts" "creative" "node" "🎨" \
    "" "net.http" "localhost" \
    "Render canvas graphics and visualizations"

scaffold_skill "memory-write" "memory-tool.ts" "data" "node" "💾" \
    "" "fs.write" "none" \
    "Write memories to persistent storage"

scaffold_skill "memory-read" "memory-tool.ts" "data" "node" "📖" \
    "" "fs.read" "none" \
    "Read memories from persistent storage"

scaffold_skill "browser-automation" "browser-tool.ts" "automation" "shell" "🌐" \
    "" "exec.shell" "full" \
    "Automate browser interactions with Playwright"

scaffold_skill "cron-schedule" "cron-tool.ts" "automation" "node" "⏰" \
    "" "net.http" "localhost" \
    "Schedule recurring tasks with cron expressions"

scaffold_skill "session-spawn" "sessions-spawn-tool.ts" "automation" "node" "🚀" \
    "" "net.http" "localhost" \
    "Spawn new agent sessions"

scaffold_skill "nodes-list" "nodes-tool.ts" "automation" "node" "📡" \
    "" "net.http" "localhost" \
    "List and manage distributed nodes"

scaffold_skill "gateway-query" "gateway-tool.ts" "code" "node" "⚙️" \
    "" "net.http" "localhost" \
    "Query the LLM gateway for model information"

scaffold_skill "cascade-run" "cascade-tool.ts" "code" "node" "🌊" \
    "" "fs.write" "none" \
    "Execute cascade workflows"

scaffold_skill "agent-step" "agent-step.ts" "code" "node" "🤖" \
    "" "net.http" "localhost" \
    "Execute a single agent reasoning step"

# ─────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────
echo ""
echo "=== Import complete ==="
echo "Registry: $REGISTRY_DIR"
echo "Skills scaffolded: 20"
echo "Run with --force to overwrite existing files."
