#!/usr/bin/env python3
"""
Carnelian Python Worker
Executes Python-based skills with sandboxing and resource limits

Implements a JSON Lines protocol over stdin/stdout, mirroring the Node worker.
The orchestrator sends Health, Invoke, and Cancel messages; this worker
responds with HealthResult, InvokeResult, and Stream messages.
"""

import json
import sys
import os
import time
import traceback
from datetime import datetime

VERSION = "0.1.0"
start_time = datetime.now()
shutting_down = False


# =============================================================================
# ATTESTATION
# =============================================================================

def compute_build_checksum():
    """Compute build checksum for attestation.

    Uses CARNELIAN_BUILD_CHECKSUM env var if set (provided by orchestrator),
    otherwise falls back to a version string.
    """
    env_checksum = os.getenv("CARNELIAN_BUILD_CHECKSUM")
    if env_checksum:
        return env_checksum
    return f"v{VERSION}"


# =============================================================================
# MESSAGE HANDLERS
# =============================================================================

def handle_health(message_id):
    """Handle health check request with attestation."""
    attestation = {
        "last_ledger_head": os.getenv("CARNELIAN_LEDGER_HEAD", "genesis"),
        "build_checksum": compute_build_checksum(),
        "config_version": os.getenv("CARNELIAN_CONFIG_VERSION", "v1"),
    }

    response = {
        "type": "HealthResult",
        "message_id": message_id,
        "payload": {
            "healthy": not shutting_down,
            "worker_id": f"python-worker-{os.getpid()}",
            "uptime_secs": int((datetime.now() - start_time).total_seconds()),
            "attestation": attestation,
        }
    }

    print(json.dumps(response), flush=True)


def handle_invoke(message_id, payload):
    """Handle skill invocation request."""
    run_id = payload.get("run_id", "unknown")
    skill_name = payload.get("skill_name", "unknown")
    start = time.monotonic()

    try:
        # TODO: Implement actual skill loading and sandboxed execution
        result = {
            "message": f"Python worker executed skill '{skill_name}'",
            "skill_name": skill_name,
        }
        duration_ms = int((time.monotonic() - start) * 1000)

        response = {
            "type": "InvokeResult",
            "message_id": message_id,
            "payload": {
                "run_id": run_id,
                "status": "Success",
                "result": result,
                "error": None,
                "exit_code": 0,
                "duration_ms": duration_ms,
                "truncated": False,
            }
        }
    except Exception as e:
        duration_ms = int((time.monotonic() - start) * 1000)
        response = {
            "type": "InvokeResult",
            "message_id": message_id,
            "payload": {
                "run_id": run_id,
                "status": "Failed",
                "result": {},
                "error": str(e),
                "exit_code": 1,
                "duration_ms": duration_ms,
                "truncated": False,
            }
        }

    print(json.dumps(response), flush=True)


def handle_cancel(message_id, payload):
    """Handle cancellation request."""
    run_id = payload.get("run_id", "unknown")
    reason = payload.get("reason", "unknown")
    log(f"Cancel requested for run {run_id}: {reason}")


# =============================================================================
# PROTOCOL
# =============================================================================

def log(message):
    """Log a message to stderr (not stdout, which is the protocol channel)."""
    print(f"[python-worker] {message}", file=sys.stderr, flush=True)


def on_message(message):
    """Route an incoming transport message to the appropriate handler."""
    msg_type = message.get("type")
    message_id = message.get("message_id", "")

    if msg_type == "Health":
        handle_health(message_id)
    elif msg_type == "Invoke":
        payload = message.get("payload", {})
        handle_invoke(message_id, payload)
    elif msg_type == "Cancel":
        payload = message.get("payload", {})
        handle_cancel(message_id, payload)
    else:
        log(f"Unknown message type: {msg_type}")


def main_loop():
    """Read JSON Lines from stdin and dispatch messages."""
    log(f"Carnelian Python Worker v{VERSION} started (pid={os.getpid()})")

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            message = json.loads(line)
            on_message(message)
        except json.JSONDecodeError as e:
            log(f"Protocol error: {e} — raw: {line[:200]}")
        except Exception as e:
            log(f"Unhandled error processing message: {e}")
            traceback.print_exc(file=sys.stderr)

    log("stdin closed — shutting down")


if __name__ == "__main__":
    main_loop()
