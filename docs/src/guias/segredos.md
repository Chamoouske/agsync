# Segredos

Tokens e chaves nunca vao no `agsync.yaml`. O arquivo cita a variavel como `${VAR}`, e cada maquina fornece o valor.

```yaml
mcps:
  context7:
    url: https://mcp.context7.com/mcp
    headers:
      CONTEXT7_API_KEY: ${CONTEXT7_API_KEY}
```

`${VAR}` vale em `command`, `args`, `env`, `url` e `headers`.

## Fornecer o valor

Um `${VAR}` e resolvido primeiro pelo ambiente do processo e depois pelo `secrets.env`.

Pelo `secrets.env`:

```sh
agsync secret set CONTEXT7_API_KEY
```

O valor e lido sem eco do terminal, ou da stdin:

```sh
printf '%s' "$TOKEN" | agsync secret set CONTEXT7_API_KEY
```

- O nome aceita letras, numeros e `_`. Valor vazio nao e gravado.
- O arquivo fica na pasta de config (veja [Arquivos e pastas](../referencia/arquivos.md)) e e acessivel so pelo usuario: permissao `0600` no Linux e uma ACL protegida com uma unica entrada para o usuario no Windows.
- `agsync secret list` mostra os nomes, nunca os valores.

## O que o agente recebe

- Valor vindo do ambiente: o agente que expande variaveis recebe a referencia `${VAR}`, nao o valor.
  - O Codex so expande em alguns campos: env com o proprio nome, header `${VAR}` e `Authorization: Bearer ${VAR}`. Nos demais campos ele recebe o valor.
  - O Antigravity nao expande `${VAR}`: o valor e gravado no arquivo.
- Valor so no `secrets.env`: o agente recebe o valor.
- Segredo ausente: o item e pulado e aparece em `status` e `doctor`.

## Garantias

- Valores de `env` e `headers` nunca aparecem na saida do agsync.
- O hash do `state.json` nao depende dos valores: trocar um segredo nao gera pendencia.
- MCP importado so le os segredos listados em `imports.mcps[].secrets`. Veja [Importar de outros repos](imports.md).
