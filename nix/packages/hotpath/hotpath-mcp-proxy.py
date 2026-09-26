"""
hotpath MCP proxy — stdio MCP server that forwards to hotpath's HTTP MCP.

The MCP client launches this as a regular stdio MCP server.  Tool calls
are forwarded to whichever hotpath endpoint is currently active.
Supports multiple named endpoints (local, remote, tunnel, …) configured
via a YAML file that is hot-reloaded on every tool call.

Config file location (first match wins):
  $HOTPATH_PROXY_CONFIG
  $REPOSITORY_ROOT/.hotpath-proxy.yaml
  .hotpath-proxy.yaml   (cwd)

Config format (YAML):
  endpoints:
    local:   http://localhost:6771/mcp
    remote:  http://remote-host:6771/mcp
  active: local

If no config file exists, falls back to http://localhost:6771/mcp.

Protocol: JSON-RPC over stdin/stdout (newline-delimited).
"""

import json
import sys
import os
import socket
import urllib.request
import urllib.error
from urllib.parse import urlparse
import yaml

# ── Config management ────────────────────────────────────────────────

DEFAULT_URL = "http://localhost:6771/mcp"
DEFAULT_CONFIG = {"endpoints": {"local": DEFAULT_URL}, "active": "local"}


def _find_config_path():
    """Locate the config file."""
    explicit = os.environ.get("HOTPATH_PROXY_CONFIG")
    if explicit and os.path.isfile(explicit):
        return explicit
    ws = os.environ.get("REPOSITORY_ROOT", "")
    if ws:
        p = os.path.join(ws, ".hotpath-proxy.yaml")
        if os.path.isfile(p):
            return p
    if os.path.isfile(".hotpath-proxy.yaml"):
        return ".hotpath-proxy.yaml"
    return None


class Config:
    """Hot-reloadable endpoint configuration."""

    def __init__(self):
        self._path = _find_config_path()
        self._mtime = 0.0
        self._data = dict(DEFAULT_CONFIG)
        self._reload()

    def _reload(self):
        if not self._path:
            return
        try:
            st = os.stat(self._path)
            if st.st_mtime == self._mtime:
                return
            self._mtime = st.st_mtime
            with open(self._path) as f:
                raw = yaml.safe_load(f) or {}
            if "endpoints" in raw and isinstance(raw["endpoints"], dict):
                self._data["endpoints"] = raw["endpoints"]
            if "active" in raw and raw["active"] in self._data["endpoints"]:
                self._data["active"] = raw["active"]
        except Exception:
            pass  # keep last good config

    def _save(self):
        path = self._path
        if not path:
            ws = os.environ.get("REPOSITORY_ROOT", "")
            path = os.path.join(ws, ".hotpath-proxy.yaml") if ws else ".hotpath-proxy.yaml"
            self._path = path
        try:
            with open(path, "w") as f:
                yaml.dump(self._data, f, default_flow_style=False, sort_keys=False)
            self._mtime = os.stat(path).st_mtime
        except Exception:
            pass

    def maybe_reload(self):
        self._reload()

    @property
    def active_name(self):
        return self._data.get("active", "local")

    @property
    def active_url(self):
        name = self.active_name
        return self._data["endpoints"].get(name, DEFAULT_URL)

    @property
    def endpoints(self):
        return dict(self._data.get("endpoints", {}))

    def set_active(self, name):
        if name in self._data["endpoints"]:
            self._data["active"] = name
            self._save()
            return True
        return False

    def add_endpoint(self, name, url):
        self._data["endpoints"][name] = url
        self._save()

    def remove_endpoint(self, name):
        if name in self._data["endpoints"]:
            del self._data["endpoints"][name]
            if self._data["active"] == name:
                self._data["active"] = next(iter(self._data["endpoints"]), "local")
            self._save()
            return True
        return False


# ── Static tool definitions (from hotpath v0.11) ────────────────────

HOTPATH_TOOLS = [
    {
        "name": "functions_timing",
        "description": "Get execution timing metrics for all instrumented functions. Returns call count, avg, p50/p95/p99, total time, and % of total runtime.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "functions_alloc",
        "description": "Get memory allocation metrics per function (requires hotpath-alloc feature). Returns bytes allocated and allocation counts.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "threads",
        "description": "Get thread CPU usage metrics. Shows thread names, status, CPU%, and cumulative CPU time.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "channels",
        "description": "Get channel metrics: sent/received counts, queue size, and channel state.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "streams",
        "description": "Get stream metrics: items yielded and stream state.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "futures",
        "description": "Get future lifecycle metrics: poll counts and state.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "gauges",
        "description": "Get gauge metrics: current/min/max values and update count.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "function_timing_logs",
        "description": "Get detailed timing logs for a specific function.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "function_name": {
                    "type": "string",
                    "description": "Name of the function to get timing logs for",
                }
            },
            "required": ["function_name"],
        },
    },
    {
        "name": "function_alloc_logs",
        "description": "Get detailed allocation logs for a specific function.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "function_name": {
                    "type": "string",
                    "description": "Name of the function to get allocation logs for",
                }
            },
            "required": ["function_name"],
        },
    },
    {
        "name": "channel_logs",
        "description": "Get message logs for a specific channel.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "channel_id": {"type": "string", "description": "ID of the channel"}
            },
            "required": ["channel_id"],
        },
    },
    {
        "name": "stream_logs",
        "description": "Get item logs for a specific stream.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "stream_id": {"type": "string", "description": "ID of the stream"}
            },
            "required": ["stream_id"],
        },
    },
    {
        "name": "future_logs",
        "description": "Get poll/completion logs for a specific future.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "future_id": {"type": "string", "description": "ID of the future"}
            },
            "required": ["future_id"],
        },
    },
    {
        "name": "gauge_logs",
        "description": "Get value update logs for a specific gauge.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "gauge_id": {"type": "string", "description": "ID of the gauge"}
            },
            "required": ["gauge_id"],
        },
    },
]

# Proxy-only tools for endpoint management
PROXY_TOOLS = [
    {
        "name": "list_endpoints",
        "description": "List all configured hotpath endpoints, which one is active, and their connectivity status.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "set_endpoint",
        "description": "Switch the active hotpath endpoint by name or add a new one.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Endpoint name to activate (e.g. 'local', 'remote'). If 'url' is also given, creates/updates the endpoint first.",
                },
                "url": {
                    "type": "string",
                    "description": "Optional URL for a new/updated endpoint (e.g. 'http://remote-host:6771/mcp').",
                },
            },
            "required": ["name"],
        },
    },
]

ALL_TOOLS = HOTPATH_TOOLS + PROXY_TOOLS


# ── Session management ───────────────────────────────────────────────

class HotpathSession:
    """Manages MCP sessions with hotpath servers, keyed by URL."""

    def __init__(self):
        self._sessions = {}  # url → session_id

    def _sid(self, url):
        return self._sessions.get(url)

    def _set_sid(self, url, sid):
        if sid:
            self._sessions[url] = sid

    def _clear_sid(self, url):
        self._sessions.pop(url, None)

    def _post(self, url, payload):
        """Send JSON-RPC to a hotpath server, parse SSE response."""
        headers = {
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
        }
        sid = self._sid(url)
        if sid:
            headers["Mcp-Session-Id"] = sid

        data = json.dumps(payload).encode()
        req = urllib.request.Request(url, data=data, headers=headers)
        resp = urllib.request.urlopen(req, timeout=10)

        new_sid = resp.headers.get("Mcp-Session-Id")
        if new_sid:
            self._set_sid(url, new_sid)

        body = resp.read().decode()
        for line in body.splitlines():
            if line.startswith("data: "):
                return json.loads(line[6:])
        if body.strip():
            return json.loads(body)
        return None

    def ensure_initialized(self, url):
        if self._sid(url):
            return True
        result = self._post(url, {
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "hotpath-mcp-proxy", "version": "0.2.0"},
            },
        })
        if result and "result" in result:
            try:
                self._post(url, {"jsonrpc": "2.0", "method": "notifications/initialized"})
            except Exception:
                pass
            return True
        return False

    def _resolve_function_id(self, url, name_query):
        result = self._post(url, {
            "jsonrpc": "2.0", "id": 99,
            "method": "tools/call",
            "params": {"name": "functions_timing", "arguments": {}},
        })
        if not result or "result" not in result:
            return None
        content = result["result"].get("content", [])
        if not content:
            return None
        try:
            data = json.loads(content[0].get("text", "{}"))
        except (json.JSONDecodeError, IndexError):
            return None
        for fn in data.get("data", []):
            if fn.get("name") == name_query:
                return fn.get("id")
            if name_query in fn.get("name", ""):
                return fn.get("id")
        return None

    _FN_ID_TOOLS = {"function_timing_logs", "function_alloc_logs"}

    def call_tool(self, url, name, arguments):
        return self._call_tool_inner(url, name, arguments, retry=True)

    def _call_tool_inner(self, url, name, arguments, retry=True):
        try:
            self.ensure_initialized(url)
            remapped = dict(arguments or {})
            if name in self._FN_ID_TOOLS and "function_name" in remapped:
                fn_name = remapped.pop("function_name")
                fn_id = self._resolve_function_id(url, fn_name)
                if fn_id is None:
                    return {
                        "content": [{"type": "text",
                                     "text": f"No function matching '{fn_name}' found."}],
                        "isError": True,
                    }
                remapped["function_id"] = fn_id
            result = self._post(url, {
                "jsonrpc": "2.0", "id": 1,
                "method": "tools/call",
                "params": {"name": name, "arguments": remapped},
            })
        except urllib.error.HTTPError as e:
            if e.code == 401 and retry:
                self._clear_sid(url)
                return self._call_tool_inner(url, name, arguments, retry=False)
            raise
        if result and "result" in result:
            return result["result"]
        if result and "error" in result:
            return {"content": [{"type": "text", "text": f"hotpath error: {result['error']}"}], "isError": True}
        return {"content": [{"type": "text", "text": "No response from hotpath"}], "isError": True}


# ── Helpers ──────────────────────────────────────────────────────────

def check_reachable(url):
    """Fast socket-level check if a hotpath URL is reachable."""
    parsed = urlparse(url)
    host = parsed.hostname or "localhost"
    port = parsed.port or 80
    try:
        s = socket.create_connection((host, port), timeout=0.5)
        s.close()
        return True
    except (ConnectionRefusedError, OSError, socket.timeout):
        return False


def write_response(msg):
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def text_result(req_id, text, is_error=False):
    write_response({
        "jsonrpc": "2.0", "id": req_id,
        "result": {"content": [{"type": "text", "text": text}], "isError": is_error},
    })


# ── Handlers ─────────────────────────────────────────────────────────

def handle_initialize(req_id):
    write_response({
        "jsonrpc": "2.0", "id": req_id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "hotpath-mcp-proxy", "version": "0.2.0"},
            "instructions": (
                "Proxy to hotpath profiler MCP. "
                "Start a profiled run with hotpath instrumentation to enable live queries. "
                "Tools: functions_timing, threads, function_timing_logs, etc. "
                "Use list_endpoints / set_endpoint to manage remote connections."
            ),
        },
    })


def handle_tools_list(req_id):
    write_response({"jsonrpc": "2.0", "id": req_id, "result": {"tools": ALL_TOOLS}})


def handle_list_endpoints(req_id, config):
    config.maybe_reload()
    lines = []
    active = config.active_name
    for name, url in config.endpoints.items():
        status = "reachable" if check_reachable(url) else "unreachable"
        marker = " (active)" if name == active else ""
        lines.append(f"  {name}{marker}: {url} [{status}]")
    text = "Configured endpoints:\n" + "\n".join(lines)
    cfg_path = config._path or "(no config file — using defaults)"
    text += f"\n\nConfig: {cfg_path}"
    text_result(req_id, text)


def handle_set_endpoint(req_id, config, arguments):
    config.maybe_reload()
    name = arguments.get("name", "")
    url = arguments.get("url")
    if not name:
        text_result(req_id, "Missing 'name' parameter.", is_error=True)
        return
    if url:
        config.add_endpoint(name, url)
    if config.set_active(name):
        reachable = check_reachable(config.active_url)
        status = "reachable" if reachable else "unreachable"
        text_result(req_id, f"Active endpoint set to '{name}': {config.active_url} [{status}]")
    else:
        available = ", ".join(config.endpoints.keys())
        text_result(req_id, f"Unknown endpoint '{name}'. Available: {available}", is_error=True)


def handle_tools_call(req_id, params, config, session):
    name = params.get("name", "")
    arguments = params.get("arguments", {})

    # Proxy-only tools
    if name == "list_endpoints":
        handle_list_endpoints(req_id, config)
        return
    if name == "set_endpoint":
        handle_set_endpoint(req_id, config, arguments)
        return

    # Hot-reload config before forwarding
    config.maybe_reload()
    url = config.active_url
    ep_name = config.active_name

    if not check_reachable(url):
        text_result(req_id, (
            f"hotpath MCP is not reachable at '{ep_name}' ({url}).\n\n"
            "To start a local profiled run, build a binary/example with the\n"
            "hotpath profiling feature enabled and run it, e.g.:\n"
            "  cargo build --profile profiling --features profiling --example <example>\n"
            "  HOTPATH_OUTPUT_FORMAT=none ./target/<target-triple>/profiling/examples/<example> <workload>\n\n"
            "Use list_endpoints to see all configured endpoints, "
            "or set_endpoint to switch."
        ), is_error=True)
        return

    try:
        result = session.call_tool(url, name, arguments)
        write_response({"jsonrpc": "2.0", "id": req_id, "result": result})
    except (urllib.error.URLError, OSError) as e:
        session._clear_sid(url)
        text_result(req_id, f"Failed to reach hotpath at '{ep_name}' ({url}): {e}", is_error=True)
    except Exception as e:
        session._clear_sid(url)
        text_result(req_id, f"Error calling hotpath at '{ep_name}': {e}", is_error=True)


# ── Main loop ────────────────────────────────────────────────────────

def main():
    config = Config()
    session = HotpathSession()

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue

        method = msg.get("method", "")
        req_id = msg.get("id")

        if method == "initialize":
            handle_initialize(req_id)
        elif method == "notifications/initialized":
            pass
        elif method == "tools/list":
            handle_tools_list(req_id)
        elif method == "tools/call":
            handle_tools_call(req_id, msg.get("params", {}), config, session)
        elif req_id is not None:
            write_response({
                "jsonrpc": "2.0", "id": req_id,
                "error": {"code": -32601, "message": f"Method not found: {method}"},
            })


if __name__ == "__main__":
    main()
