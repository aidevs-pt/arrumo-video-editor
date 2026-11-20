# Connect to Arrumo

[Español](../es/connect.md) · [Português](../pt/connect.md)

## Create a test workspace

From the editor repository, run these commands in a POSIX shell. Core-only mode builds the local interface without the private default plugin checkouts. Use the new temporary database for all three examples.

```sh
ARRUMO_CORE_ONLY=1 cargo build --locked -p arrumo-workspaces
editor="$PWD/target/debug/arrumo-workspaces"
database="$(mktemp -d)/workspaces.sqlite"
"$editor" "$database" create "Studio"
"$editor" "$database" list
```

Both commands print JSON. Keep the workspace ID for operations that take a workspace scope.

## MCP

For a persistent MCP client, configure the binary as the command and pass the absolute database path and `mcp` as arguments. The following stdio session lists the tools and calls `list_workspaces`. Read the final response’s `result.structuredContent.workspaces`.

```sh
cat <<'JSON' | "$editor" "$database" mcp
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"arrumo-quickstart","version":"1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_workspaces","arguments":{}}}
JSON
```

## A2A

The A2A server binds to loopback. It reads the bearer token from stdin and prints its endpoint. This example keeps the token inside a local Python process and shuts down the server after the query.

```sh
python3 - "$editor" "$database" <<'PYTHON'
import json, secrets, subprocess, sys
from urllib.request import Request, urlopen

editor, database = sys.argv[1:]
token = secrets.token_hex(32)
with subprocess.Popen(
    [editor, database, "a2a", "0"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
) as server:
    try:
        server.stdin.write(token + "\n")
        server.stdin.close()
        endpoint = server.stdout.readline().strip()
        body = {"jsonrpc": "2.0", "id": 1, "method": "SendMessage",
                "params": {"message": {
                    "messageId": "arrumo-quickstart", "role": "ROLE_USER",
                    "parts": [{"data": {"operation": "list_workspaces", "arguments": {}}}],
                }}}
        request = Request(endpoint, json.dumps(body).encode(), {
            "Authorization": "Bearer " + token,
            "Content-Type": "application/json",
            "A2A-Version": "1.0",
        })
        with urlopen(request, timeout=10) as response:
            data = json.load(response)["result"]["message"]["parts"][0]["data"]
            print(json.dumps(data["result"], indent=2))
    finally:
        server.terminate()
PYTHON
```

For operation names and schemas, send the same message with `list_operations`. Anyone with the bearer token can act with the local operator’s database permissions.

## Reference

See the [editor reference](../../../README.md) for generation, review, media, and plugin commands. See the [Plugin SDK](https://github.com/aidevs-pt/arrumo-plugin-sdk) for plugin authoring and the [official registry](https://github.com/aidevs-pt/arrumo-plugin-registry/blob/main/docs/en/plugins.md) for default plugin repositories and release metadata.
