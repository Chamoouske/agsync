# Segredos

Tokens e chaves nunca vão no `agsync.yaml`. O arquivo cita a variável como `${VAR}`, e cada máquina fornece o valor.

```yaml
mcps:
  context7:
    url: https://mcp.context7.com/mcp
    headers:
      CONTEXT7_API_KEY: ${CONTEXT7_API_KEY}
```

`${VAR}` vale em `command`, `args`, `env`, `url` e `headers`.

## Fornecer o valor

Um `${VAR}` é resolvido primeiro pelo ambiente do processo e depois pelo `secrets.env`.

Pelo `secrets.env`:

```sh
agsync secret set CONTEXT7_API_KEY
```

O valor é lido sem eco do terminal, ou da stdin:

```sh
printf '%s' "$TOKEN" | agsync secret set CONTEXT7_API_KEY
```

- O nome aceita letras, números e `_`. Valor vazio não é gravado.
- O arquivo fica na pasta de config (veja [Arquivos e pastas](../referencia/arquivos.md)) e é acessível só pelo usuário: permissão `0600` no Linux e uma ACL protegida com uma única entrada para o usuário no Windows.
- `agsync secret list` mostra os nomes, nunca os valores.

## O que o agente recebe

- Valor vindo do ambiente: o agente que expande variáveis recebe a referência `${VAR}`, não o valor.
  - O Codex só expande em alguns campos: env com o próprio nome, header `${VAR}` e `Authorization: Bearer ${VAR}`. Nos demais campos ele recebe o valor.
  - O Antigravity não expande `${VAR}`: o valor é gravado no arquivo.
- Valor só no `secrets.env`: o agente recebe o valor.
- Segredo ausente: o item é pulado e aparece em `status` e `doctor`.

## Garantias

- Valores de `env` e `headers` nunca aparecem na saída do agsync.
- O hash do `state.json` não depende dos valores: trocar um segredo não gera pendência.
- MCP importado só lê os segredos listados em `imports.mcps[].secrets`. Veja [Importar de outros repos](imports.md).
