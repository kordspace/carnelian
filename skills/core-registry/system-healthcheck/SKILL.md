---
name: system-healthcheck
description: "Host security hardening and risk-tolerance configuration for Carnelian deployments. Use when a user asks for security audits, firewall/SSH/update hardening, risk posture, exposure review, or version status checks on a machine running Carnelian (laptop, workstation, Pi, VPS)."
metadata:
  openclaw:
    emoji: "💓"
  carnelian:
    runtime: shell
    version: "1.0.0"
    sandbox:
      network: localhost
      resourceLimits:
        maxMemoryMB: 512
        maxCpuPercent: 50
        timeoutSecs: 300
    capabilities:
      - exec.shell
      - fs.read
---

# Carnelian Host Hardening

## Overview

Assess and harden the host running Carnelian, then align it to a user-defined risk tolerance without breaking access. Use system security tooling as a first-class signal, but treat OS hardening as a separate, explicit set of steps.

## Core rules

- Recommend running this skill with a state-of-the-art model (e.g., Opus 4.5, GPT 5.2+). The agent should self-check the current model and suggest switching if below that level; do not block execution.
- Require explicit approval before any state-changing action.
- Do not modify remote access settings without confirming how the user connects.
- Prefer reversible, staged changes with a rollback plan.
- Never claim Carnelian changes the host firewall, SSH, or OS updates; it does not.
- If role/identity is unknown, provide recommendations only.
- Formatting: every set of user choices must be numbered so the user can reply with a single digit.
- System-level backups are recommended; try to verify status.

## Workflow (follow in order)

### 0) Model self-check (non-blocking)

Before starting, check the current model. If it is below state-of-the-art (e.g., Opus 4.5, GPT 5.2+), recommend switching. Do not block execution.

### 1) Establish context (read-only)

Try to infer 1-5 from the environment before asking. Prefer simple, non-technical questions if you need confirmation.

Determine (in order):

1. OS and version (Linux/macOS/Windows), container vs host.
2. Privilege level (root/admin vs user).
3. Access path (local console, SSH, RDP, tailnet).
4. Network exposure (public IP, reverse proxy, tunnel).
5. Backup system and status (e.g., Time Machine, system images, snapshots).
6. Deployment context (local mac app, headless host, remote server, container/CI).
7. Disk encryption status (FileVault/LUKS/BitLocker).
8. OS automatic security updates status.
9. Usage mode for a personal assistant with full access.

### 2) Run system security audits (read-only)

Check system security posture:
- Firewall status and rules
- Open ports and listening services
- SSH configuration (if applicable)
- User accounts and permissions
- Installed security updates
- Running processes and services

### 3) Check Carnelian version/update status (read-only)

Check Carnelian installation:
- Current version
- Available updates
- Configuration status
- Worker health

### 4) Determine risk tolerance (after system context)

Offer suggested profiles:

1. Home/Workstation Balanced
2. VPS Hardened
3. Developer Convenience
4. Custom

### 5) Produce a remediation plan

Include target profile, current posture summary, gaps, step-by-step remediation, rollback plan, and risks.

### 6) Offer execution options

1. Do it for me (guided, step-by-step approvals)
2. Show plan only
3. Fix only critical issues
4. Export commands for later

### 7) Execute with confirmations

For each step: show command, explain impact, confirm access, stop on unexpected output.

### 8) Verify and report

Re-check firewall, ports, remote access, and system security. Deliver final posture report.
