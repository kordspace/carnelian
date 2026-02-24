#!/bin/sh
# ================================
# Claude CLI Agent Template for Traycer
# Auto-accepts all actions (YOLO mode)
# ================================

claude --dangerously-skip-permissions --append-system-prompt "$TRAYCER_SYSTEM_PROMPT" "$TRAYCER_PROMPT"
