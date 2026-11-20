# Liga-te ao Arrumo

[English](../en/connect.md) · [Español](../es/connect.md)

## Cria um espaço de trabalho de teste

Executa estes comandos a partir do repositório do editor numa shell POSIX. O modo básico compila a interface local sem os repositórios privados dos plugins predefinidos. Usa a mesma base de dados temporária nos três exemplos.

```sh
ARRUMO_CORE_ONLY=1 cargo build --locked -p arrumo-workspaces
editor="$PWD/target/debug/arrumo-workspaces"
database="$(mktemp -d)/workspaces.sqlite"
"$editor" "$database" create "Studio"
"$editor" "$database" list
```

Os dois comandos devolvem JSON. Guarda o identificador do espaço de trabalho para as operações que o exigirem.

## MCP

Para um cliente MCP persistente, configura o executável como comando e passa o caminho absoluto da base de dados e `mcp` como argumentos. A sessão seguinte usa a entrada e saída padrão para listar as ferramentas e chamar `list_workspaces`. Consulta `result.structuredContent.workspaces` na última resposta.

```sh
cat <<'JSON' | "$editor" "$database" mcp
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"arrumo-quickstart","version":"1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_workspaces","arguments":{}}}
JSON
```

## A2A

O servidor A2A escuta na interface de loopback. Lê o token de acesso da entrada padrão e apresenta o endereço do serviço. Este exemplo mantém o token dentro de um processo local de Python e termina o servidor após a consulta.

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

Para descobrir os nomes e esquemas das operações, envia a mesma mensagem com `list_operations`. Quem tiver o token pode atuar com as permissões do operador local sobre a base de dados.

## Referência

Consulta a [referência do editor](../../../README.md) para os comandos de geração, revisão, multimédia e plugins. O [SDK de plugins](https://github.com/aidevs-pt/arrumo-plugin-sdk) explica como os criar e o [registo oficial](https://github.com/aidevs-pt/arrumo-plugin-registry) liga aos repositórios e aos metadados das versões.
