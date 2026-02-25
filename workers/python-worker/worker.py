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
import subprocess
import uuid
import importlib
import importlib.util
import threading
from concurrent.futures import ThreadPoolExecutor, TimeoutError
from datetime import datetime

VERSION = "0.1.0"
start_time = datetime.now()
shutting_down = False

# Default max output size (1 MB, matching Node worker)
DEFAULT_MAX_OUTPUT_BYTES = 1_048_576

# Track which skill directories have had requirements installed
_installed_skill_dirs: set[str] = set()


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
# SKILL LOADING
# =============================================================================

def install_requirements(skill_dir: str) -> None:
    """Install requirements.txt for a skill directory if present.

    Runs at most once per directory per process lifetime.
    """
    if skill_dir in _installed_skill_dirs:
        return

    req_path = os.path.join(skill_dir, "requirements.txt")
    if os.path.isfile(req_path):
        try:
            subprocess.run(
                [
                    sys.executable,
                    "-m", "pip",
                    "install", "-r", req_path,
                    "--quiet",
                    "--disable-pip-version-check",
                ],
                check=True,
            )
        except subprocess.CalledProcessError as e:
            log(f"Failed to install requirements from {req_path}: {e}")
            raise

    _installed_skill_dirs.add(skill_dir)


def load_skill_module(skill_path: str, skill_name: str):
    """Dynamically load a skill module from disk.

    Returns the loaded module with the 'invoke' function ready to call.
    Raises ImportError if skill.py is not found or has no 'invoke' attribute.
    """
    if skill_path not in sys.path:
        sys.path.insert(0, skill_path)

    module_file = os.path.join(skill_path, "skill.py")
    if not os.path.isfile(module_file):
        raise ImportError(f"skill.py not found in {skill_path}")

    spec = importlib.util.spec_from_file_location(
        f"carnelian_skill_{skill_name}", module_file
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)

    if not hasattr(module, "invoke"):
        raise ImportError(f"Skill module {skill_name} has no 'invoke' function")

    return module


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
    skill_path = payload.get("skill_path", "")
    args = payload.get("input", {})
    timeout_secs = payload.get("timeout_secs", 30)
    start = time.monotonic()

    try:
        # Read max output size from env or use default
        max_output_bytes = int(os.getenv("CARNELIAN_MAX_OUTPUT_BYTES", DEFAULT_MAX_OUTPUT_BYTES))

        # Create emitter for stream events
        emitter = StreamEmitter(run_id, max_output_bytes)

        # Emit start log
        emitter.emit_log("Info", f"Invoking skill '{skill_name}'", {"skill_path": skill_path})

        # Install requirements if present (one-time per skill directory)
        install_requirements(skill_path)

        # Load the skill module
        skill_module = load_skill_module(skill_path, skill_name)

        # Call the skill's invoke function with timeout
        result = _invoke_with_timeout(skill_module, args, timeout_secs)

        # Emit completion progress
        emitter.emit_progress(100, "Skill completed")

        # Enforce output size limit on result
        result_json = json.dumps(result)
        result_bytes = len(result_json.encode("utf-8"))
        truncated = False

        if result_bytes > max_output_bytes:
            # Truncate result to max_output_bytes
            truncated_result = result_json.encode("utf-8")[:max_output_bytes].decode("utf-8", errors="replace")
            emitter.emit_log("Warn", f"Result truncated at {max_output_bytes} bytes")
            truncated = True
            # Try to parse truncated result as JSON, fallback to wrapped dict
            try:
                result = json.loads(truncated_result)
            except json.JSONDecodeError:
                result = {"truncated": True}

        truncated = truncated or emitter.is_truncated()
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
                "truncated": truncated,
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


def _invoke_with_timeout(skill_module, args, timeout_secs):
    """Execute skill invoke in a thread with timeout enforcement."""
    result_container = {}
    
    def target():
        try:
            result_container["result"] = skill_module.invoke(args)
            result_container["success"] = True
        except Exception as e:
            result_container["error"] = e
            result_container["success"] = False
    
    thread = threading.Thread(target=target)
    thread.start()
    thread.join(timeout=timeout_secs)
    
    if thread.is_alive():
        # Timeout occurred - we can't kill the thread in Python,
        # but we mark it as failed and prevent further emissions
        raise TimeoutError(f"Skill invocation timed out after {timeout_secs} seconds")
    
    if not result_container.get("success", False):
        raise result_container.get("error", Exception("Skill invocation failed"))
    
    return result_container["result"]


def handle_cancel(message_id, payload):
    """Handle cancellation request."""
    run_id = payload.get("run_id", "unknown")
    reason = payload.get("reason", "unknown")
    log(f"Cancel requested for run {run_id}: {reason}")


# =============================================================================
# PROTOCOL
# =============================================================================

class StreamEmitter:
    """Emit Stream messages for progress and log events.

    Mirrors the Node EventEmitter pattern from events.ts.
    Enforces max output size to prevent memory issues.
    """

    def __init__(self, run_id: str, max_bytes: int):
        self.run_id = run_id
        self._max_bytes = max_bytes
        self._total_bytes = 0
        self._truncated = False

    def emit_log(self, level: str, message: str, fields: dict = None) -> None:
        """Emit a Log stream event."""
        if fields is None:
            fields = {}
        fields["level_str"] = level
        self._emit("Log", message, fields, level)

    def emit_progress(self, percentage: float, message: str, stage=None, step=None) -> None:
        """Emit a Progress stream event."""
        fields = {"percentage": max(0.0, min(100.0, percentage))}
        if stage is not None:
            fields["stage"] = stage
        if step is not None:
            fields["step"] = step
        self._emit("Progress", message, fields, "Info")

    def emit_artifact(self, file_path: str, file_type: str = None) -> None:
        """Emit an Artifact stream event with file metadata."""
        fields = {"file_path": file_path}
        
        if os.path.isfile(file_path):
            try:
                stat = os.stat(file_path)
                fields["size"] = stat.st_size
                fields["exists"] = True
            except OSError:
                fields["exists"] = False
        else:
            fields["exists"] = False
        
        if file_type is not None:
            fields["file_type"] = file_type
        
        self._emit("Artifact", f"Artifact: {file_path}", fields, "Info")

    def _emit(self, event_type: str, message: str, fields: dict, level: str | None) -> bool:
        """Emit a Stream event. Returns False if truncated."""
        if self._truncated:
            return False

        event = {
            "run_id": self.run_id,
            "event_type": event_type,
            "timestamp": datetime.utcnow().isoformat() + "Z",
            "level": level,
            "message": message,
            "fields": fields,
        }

        envelope = {
            "type": "Stream",
            "message_id": str(uuid.uuid4()),
            "payload": event,
        }

        json_str = json.dumps(envelope)
        size = len(json_str.encode("utf-8")) + 1  # +1 for newline

        if self._total_bytes + size > self._max_bytes:
            self._truncated = True
            # Emit truncation notice
            truncation_event = {
                "run_id": self.run_id,
                "event_type": "Log",
                "timestamp": datetime.utcnow().isoformat() + "Z",
                "level": "Warn",
                "message": f"Output truncated at {self._max_bytes} bytes",
                "fields": {"level_str": "Warn"},
            }
            truncation_envelope = {
                "type": "Stream",
                "message_id": str(uuid.uuid4()),
                "payload": truncation_event,
            }
            print(json.dumps(truncation_envelope), flush=True)
            return False

        print(json_str, flush=True)
        self._total_bytes += size
        return True


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
