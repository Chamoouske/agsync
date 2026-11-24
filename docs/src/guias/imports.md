# Importar de outros repos

O campo `imports` junta ao repo principal os MCPs, skills ou plugins do `agsync.yaml` de outros repos de config. Serve, por exemplo, para um time manter um repo de MCPs comuns e cada pessoa importa-lo do seu repo pessoal.

```yaml
imports:
  mcps:
    - git: https://github.com/<org>/mcps-work
      ref: main
      profiles: [work]
      excludes: [mcp-test]
      secrets: [GITHUB_TOKEN]
  skills:
    - git: https://github.com/<org>/skills-personal
      ref: 8f3a2c1e5b7d9f0a1c3e5b7d9f0a1c3e5b7d9f0a
      profiles: [personal]
      excludes: [tdd]
  plugins:
    - git: https://github.com/<org>/plugins-work
      ref: v1.2.0
      profiles: [work]
      excludes: [plugin-test]
```

Cada entrada importa so um tipo (`mcps`, `skills` ou `plugins`) do `agsync.yaml` na raiz do repo referenciado. O mesmo repo pode aparecer em mais de um tipo. Os campos estao em [agsync.yaml](../referencia/agsync-yaml.md#imports).

## Liberar segredos para MCPs importados

Sem `secrets`, nenhum MCP importado le segredo. Um MCP importado com `${VAR}` fora da lista e pulado, e a mensagem diz a variavel e como liberar:

```yaml
imports:
  mcps:
    - git: https://github.com/<org>/mcps-work
      secrets: [GITHUB_TOKEN]
```

`secrets` so vale em `imports.mcps`.

## O que vem do repo importado

- Os itens importados assumem os `profiles` da entrada de import. Os `profiles` declarados no repo importado sao ignorados; os `agents` sao mantidos.
- Skill em pasta local do repo importado e baixada do proprio repo importado, no commit resolvido.
- Plugin com marketplace em pasta local e pulado e aparece nos pulados.
- Skill com `git` ou MCP com `source.git` apontando para caminho local e pulado: o caminho so faz sentido no repo de origem.
- So um nivel: os `imports` do repo importado nao sao seguidos, com aviso.
- `silentUpdate` e `profiles` do repo importado sao ignorados.
- Import de perfil inativo nem e acessado na rede.

## Conflito de nomes

Entre itens do mesmo tipo ativos na maquina:

- O repo principal vence qualquer import.
- Entre imports, vence o declarado primeiro em `imports.<tipo>`.
- O perdedor e descartado com aviso nomeando as duas origens.
- Item do principal de perfil inativo nao conflita.

Exemplo: o perfil `personal` importa a skill `tdd` de um repo e o `work` importa `tdd` de outro. Numa maquina so com `personal` nao ha conflito. Com os dois perfis ativos, vence o import declarado primeiro.

## Na confirmacao

- `status`, `diff` e a confirmacao do `apply` mostram a origem de cada item importado, por exemplo `(de imports.mcps https://github.com/<org>/mcps-work@main)`.
- A lista de builds mostra a origem do build de MCP importado.
- Build de MCP importado sempre pede confirmacao: `--silent-update` e `silentUpdate` nao valem nesse caso. `apply --yes` explicito continua valendo.

## Erros

Repo inacessivel, ref inexistente ou `agsync.yaml` ausente ou invalido no repo importado fazem `status`, `diff`, `apply` e `check` falharem. A mensagem nomeia o tipo, o `git` e o `ref` do import, e nada e aplicado nem removido.
