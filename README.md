# agsync

O agsync aplica MCP servers, skills e plugins de um repositório git de configuração (o `agsync.yaml`) na configuração de usuário dos agentes de código da máquina: Claude Code, Codex, OpenCode, Hermes e Antigravity CLI.

- Só mexe no que ele mesmo escreveu; entradas manuais com o mesmo nome ficam como estão, a não ser com `--adopt`.
- Escreve pelo CLI do agente quando dá; senão, edita o arquivo preservando comentários.
- Faz backup antes de cada mudança.
- Um `agsync check` periódico só lê o remoto e avisa. Nada é aplicado sem `agsync apply`.

**Documentação completa: <https://chamoouske.github.io/agsync/>** (fonte em [`docs/`](docs/src/SUMMARY.md)).

## Instalação

Requisito: `git` no PATH.

Windows (PowerShell, sem admin; instala em `%LOCALAPPDATA%\agsync\bin`):

```powershell
irm https://raw.githubusercontent.com/Chamoouske/agsync/master/install.ps1 | iex
```

Scoop:

```powershell
scoop install https://github.com/Chamoouske/agsync/releases/latest/download/agsync.json
```

Linux x86_64 (instala em `~/.local/bin`):

```sh
curl -fsSL https://raw.githubusercontent.com/Chamoouske/agsync/master/install.sh | sh
```

Os scripts conferem o SHA256 com o `SHA256SUMS` da release. Versão específica, Cargo e mirror: veja [Instalação](docs/src/primeiros-passos/instalacao.md).

## Exemplo mínimo

No repo de config, `agsync.yaml`:

```yaml
version: 1
profiles: [work]

mcps:
  context7:
    url: https://mcp.context7.com/mcp
    headers:
      CONTEXT7_API_KEY: ${CONTEXT7_API_KEY}

skills:
  - path: skills/grug-brain-code
```

Em cada máquina:

```sh
agsync secret set CONTEXT7_API_KEY
agsync init https://github.com/<org>/agent-config --profile work
agsync status
```

O `init` grava o `machine.toml`, registra o `check` periódico, oferece o hook de shell e roda o primeiro `apply`. Depois, `agsync diff` mostra o que mudou no repo e `agsync apply` aplica.

## Microsoft Defender e SmartScreen

O binário do Windows não tem assinatura de código, e o Microsoft Defender pode por o `agsync.exe` em quarentena (por exemplo, como `Trojan:Win32/Bearfoos.A!ml`) ou o SmartScreen pode avisar sobre um aplicativo desconhecido. É um falso positivo conhecido e ainda acontece nas versões atuais, mas confira antes de liberar:

1. Compare o SHA256 com a linha de `agsync-x86_64-pc-windows-msvc.exe` no `SHA256SUMS` da [release](https://github.com/Chamoouske/agsync/releases):

   ```powershell
   (Get-FileHash "$env:LOCALAPPDATA\agsync\bin\agsync.exe" -Algorithm SHA256).Hash.ToLower()
   ```

2. Restaure da quarentena em Segurança do Windows > Proteção contra vírus e ameaças > Histórico de proteção, ou rode o instalador de novo.
3. Se voltar a bloquear, exclua só o arquivo, em um PowerShell como administrador: `Add-MpPreference -ExclusionPath "$env:LOCALAPPDATA\agsync\bin\agsync.exe"`.
4. Envie o arquivo como falso positivo em <https://www.microsoft.com/wdsi/filesubmission>.

Detalhes e riscos em [Solução de problemas](docs/src/solucao-de-problemas.md#microsoft-defender-e-smartscreen).

## Contribuindo

PRs vão para `stage`; hotfixes para `master`. Resumo em [CONTRIBUTING.md](CONTRIBUTING.md); compilar, testar, CI e releases em [Contribuição](docs/src/contribuicao/desenvolvimento.md). Mudanças por versão no [CHANGELOG.md](CHANGELOG.md).

## Licença

[MIT](LICENSE).
