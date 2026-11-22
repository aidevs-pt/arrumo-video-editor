# Conecta con Arrumo

[English](../en/connect.md) · [Português](../pt/connect.md)

## Crea un espacio de trabajo de prueba

Ejecuta estos comandos desde el repositorio del editor en una shell POSIX. El modo básico compila la interfaz local sin los repositorios privados de los complementos predeterminados. Usa la misma base de datos temporal en los tres ejemplos.

```sh
ARRUMO_CORE_ONLY=1 cargo build --locked -p arrumo-workspaces
editor="$PWD/target/debug/arrumo-workspaces"
database="$(mktemp -d)/workspaces.sqlite"
"$editor" "$database" create "Studio"
"$editor" "$database" list
```

Los dos comandos devuelven JSON. Guarda el identificador del espacio de trabajo para las operaciones que lo requieran.

## MCP

Para un cliente MCP persistente, configura el ejecutable como comando y pasa como argumentos la ruta absoluta de la base de datos y `mcp`. La siguiente sesión por entrada y salida estándar enumera las herramientas y llama a `list_workspaces`. Consulta `result.structuredContent.workspaces` en la última respuesta.

```sh
cat <<'JSON' | "$editor" "$database" mcp
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"arrumo-quickstart","version":"1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_workspaces","arguments":{}}}
JSON
```

## A2A

El servidor A2A escucha en la interfaz de bucle local. Lee el token de acceso desde la entrada estándar y muestra la dirección del servicio. Este ejemplo mantiene el token dentro de un proceso local de Python y detiene el servidor al terminar la consulta.

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

Para consultar los nombres y esquemas de las operaciones, envía el mismo mensaje con `list_operations`. Cualquiera que tenga el token puede actuar con los permisos del operador local sobre la base de datos.

## Referencia

Consulta la [referencia del editor](../../../README.md) para los comandos de generación, revisión, archivos multimedia y complementos. El [SDK de complementos](https://github.com/aidevs-pt/arrumo-plugin-sdk) explica cómo crearlos y el [registro oficial](https://github.com/aidevs-pt/arrumo-plugin-registry/blob/main/docs/es/plugins.md) enlaza los repositorios y los metadatos de las versiones.
