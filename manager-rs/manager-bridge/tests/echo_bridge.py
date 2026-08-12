#!/usr/bin/env python3
"""manager-bridge test sahtesi: gerçek qbittorrent_backend.py yerine geçer.

Satır-delimited JSON-RPC 2.0 (--bridge modunun aynısı). Gerçek Python
bağımlılıkları (pygame vb.) çekmeden köprü istemcisinin round-trip'ini test eder.
"""
import json
import sys


def handle(method: str, params: dict):
    if method == "ping":
        return "pong"
    if method == "status":
        return {"state": "STOPPED", "available": True}
    if method == "is_available":
        return True
    if method == "ensure_running":
        return bool(params.get("timeout", 0))
    if method == "get_webui_url":
        return "http://localhost:18572/"
    if method == "get_password_status":
        return {"status": "default"}
    if method == "change_webui_password":
        pw = params.get("password", "")
        if len(pw) < 8:
            return [False, "password_too_short"]
        return [True, "password_changed"]
    return None


def main() -> int:
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            sys.stdout.write(json.dumps(
                {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "Parse error"}}
            ) + "\n")
            sys.stdout.flush()
            continue
        if msg.get("method") == "shutdown":
            return 0
        result = handle(msg.get("method", ""), msg.get("params", {}) or {})
        if result is None:
            sys.stdout.write(json.dumps(
                {"jsonrpc": "2.0", "id": msg.get("id"), "error": {
                    "code": -32601, "message": f"Method not found: {msg.get('method')}"}}
            ) + "\n")
        else:
            sys.stdout.write(json.dumps(
                {"jsonrpc": "2.0", "id": msg.get("id"), "result": result}
            ) + "\n")
        sys.stdout.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())
