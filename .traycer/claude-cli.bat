@echo off
REM ================================
REM Claude CLI Agent Template for Traycer
REM Auto-accepts all actions (YOLO mode)
REM ================================

claude --dangerously-skip-permissions --append-system-prompt "%TRAYCER_SYSTEM_PROMPT%" "%TRAYCER_PROMPT%"
