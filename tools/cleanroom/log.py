#!/usr/bin/env python3
"""PostToolUse hook: appends one 'allowed' provenance entry per tool call.

Always exits 0 -- a failure here must never wedge the flight (SPEC S2.3).
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import provenance  # noqa: E402


def main() -> int:
    hook_input = {}
    try:
        hook_input = provenance.read_hook_input()
        root = provenance.repo_root(hook_input)
        path = provenance.log_path(root)
        tool_name = hook_input.get("tool_name", "") if isinstance(hook_input, dict) else ""
        tool_input = hook_input.get("tool_input", {}) if isinstance(hook_input, dict) else {}
        ctx = provenance.context(hook_input, root)
        entry = {
            "ts": provenance.now_iso(),
            "foundry_stage": ctx["foundry_stage"],
            "task_id": ctx["task_id"],
            "model": ctx["model"],
            "tool": tool_name,
            "target": provenance.target_of(tool_name, tool_input),
            "decision": "allowed",
            "reason": None,
        }
        session_id = hook_input.get("session_id") if isinstance(hook_input, dict) else None
        if session_id:
            entry["session_id"] = session_id
        agent_type = hook_input.get("agent_type") if isinstance(hook_input, dict) else None
        if agent_type:
            entry["agent_type"] = agent_type
        provenance.append(entry, path)
    except Exception as exc:  # noqa: BLE001
        try:
            root = provenance.repo_root(hook_input if isinstance(hook_input, dict) else {})
            path = provenance.log_path(root)
            provenance.append(
                {
                    "ts": provenance.now_iso(),
                    "foundry_stage": None,
                    "task_id": None,
                    "model": None,
                    "tool": "unknown",
                    "target": "",
                    "decision": "allowed",
                    "reason": f"logger-error: {exc}",
                },
                path,
            )
        except Exception as exc2:  # noqa: BLE001
            sys.stderr.write(f"cleanroom log.py: logger-error: {exc}; also failed: {exc2}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
