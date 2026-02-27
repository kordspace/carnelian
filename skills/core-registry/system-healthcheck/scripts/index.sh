#!/usr/bin/env bash
# system-healthcheck skill wrapper
# Category: system
# Carnelian host security hardening and risk assessment

set -euo pipefail

# This is a shell skill - the agent will be provided with the full skill documentation
# and should execute system commands interactively based on the workflow defined in SKILL.md

# For shell skills, the input is passed as environment variables:
# - CARNELIAN_INPUT contains the JSON input
# - CARNELIAN_RUN_ID contains the run ID
# - CARNELIAN_SKILL_NAME contains the skill name

# Parse input (if needed)
INPUT="${CARNELIAN_INPUT:-{}}"

# Output a message indicating this is an interactive skill
cat <<EOF
{
  "status": "interactive",
  "message": "This is an interactive security hardening skill. The agent will guide you through the workflow defined in the skill documentation.",
  "workflow": [
    "Model self-check (non-blocking)",
    "Establish context (read-only)",
    "Run system security audits (read-only)",
    "Check Carnelian version/update status (read-only)",
    "Determine risk tolerance",
    "Produce a remediation plan",
    "Offer execution options",
    "Execute with confirmations",
    "Verify and report"
  ],
  "note": "The agent will execute system commands on your behalf with explicit approval at each step."
}
EOF
