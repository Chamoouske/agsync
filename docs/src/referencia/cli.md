# Comandos

Visao de cada comando. A saida exata de `--help` de cada um esta em [Ajuda da CLI (gerada)](cli-help.md).

| comando | o que faz |
|---|---|
| `agsync init [<url>] [--ref R] [--profile P]... [--yes] [--silent-update]` | configura a maquina: `machine.toml`, acesso ao repo, agendamento, hook de shell e primeiro `apply`; pergunta a URL e os perfis que faltarem |
| `agsync status` | commit aplicado e remoto, mudancas pendentes, desvios, conflitos e itens pulados |
| `agsync diff` | o que o proximo `apply` muda, por agente |
| `agsync apply [--yes] [--adopt]` | mostra o diff e os builds, pede confirmacao e aplica |
| `agsync rollback [--yes]` | reaplica o commit aplicado antes do atual, pelo mesmo fluxo de plano e apply |
| `agsync check` | so le o remoto: grava `pending.json` e notifica uma vez por novidade; antes, procura versao nova do agsync; roda pelo agendador |
| `agsync hook` | imprime a linha de pendencia e a de versao nova, sem rede; roda pelo perfil do shell |
| `agsync hook install [--yes]` | poe o hook nos perfis de PowerShell, pwsh, bash, zsh e fish |
| `agsync hook remove` | tira o bloco do agsync de todos os perfis de shell |
| `agsync schedule install` | registra o `check` periodico a cada `check_interval_minutes` |
| `agsync schedule remove` | apaga o `check` periodico |
| `agsync schedule status` | diz se o `check` esta registrado e roda este binario |
| `agsync secret set NOME` | grava um valor no `secrets.env`, lido sem eco ou da stdin |
| `agsync secret list` | lista os nomes do `secrets.env`, nunca os valores |
| `agsync doctor` | confere git, repo, agentes, `requires`, segredos, permissoes, agendamento e repos de fonte |
| `agsync update [--yes]` | troca o binario pela ultima release; instalado pelo Scoop, so diz para rodar `scoop update agsync` |
| `agsync migrate-legacy [--yes]` | backup e remocao do sistema antigo (APM, 1MCP, agent-config) |
| `agsync uninstall [--yes] [--purge]` | tira dos agentes tudo o que esta no state, remove agendamento e hooks e apaga `state.json` e `pending.json` |

`agsync --version` imprime a versao; `agsync help <comando>` e `agsync <comando> --help` imprimem a ajuda.

## Confirmacao e terminal

`apply`, `rollback`, `migrate-legacy`, `uninstall`, `update` e `hook install` perguntam antes de mudar algo. Sem terminal (stdin que nao e TTY), eles so rodam com `--yes`.

Responder nao a confirmacao encerra o comando sem mudar nada e com codigo `0`.

## Linha de pendencia

Todo comando, menos `hook`, `check`, `apply` e `update`, imprime antes na stderr a linha de pendencia e a de versao nova, se houver. O `hook` imprime as mesmas linhas na stdout.

## init

- `<url>`: URL git do repo de config. Perguntada quando omitida.
- `--ref <branch|tag|sha>`: o que seguir; padrao `main`. SHA precisa ser completo.
- `--profile P`: perfil ativo; repita para varios. Perguntado quando omitido.
- `--yes`: instala o hook e aplica sem perguntar.
- `--silent-update`: aplica sem perguntar, tambem sem terminal; o hook continua perguntando. Nao vale para build de MCP importado.

Passos: `[1/5] machine.toml`, acesso ao repo, agendamento, hook, primeiro `apply`. Com o repo inacessivel, o `apply` e pulado com um aviso. Veja [Primeiro uso](../primeiros-passos/primeiro-uso.md).

## apply

- `--yes`: aplica sem perguntar; obrigatorio sem terminal.
- `--adopt`: substitui instalacoes manuais (MCP, skill, plugin) com mesmo nome, tipo e agente de um item do repo pela definicao do repo, e passa a gerencia-las. Veja [Assumir instalacoes manuais](../guias/adotar.md).

## rollback

Planeja e aplica o commit guardado em `previous` no `state.json`. Falha se nao ha commit anterior. Veja [Voltar para o commit anterior](../guias/rollback.md).

## check

Nao pergunta nada e nunca aplica. Se outra execucao segura o lock, sai na hora, em silencio. Remoto inacessivel vai para o log e nao e erro. Cada evento vira uma linha em `<dados>/logs/check.log`. Veja [Ciclo check, hook e apply](../explicacao/ciclo.md).

## hook

Sem subcomando: imprime as linhas de pendencia e de versao nova e, se nenhum `check` teve sucesso ha mais que o dobro do intervalo, dispara um `check` em segundo plano. Nunca acessa a rede.

O bloco instalado nos perfis fica entre as linhas `# >>> agsync hook >>>` e `# <<< agsync hook <<<` e so chama o `agsync hook` se o binario existir. Perfis:

- PowerShell 5.1 e PowerShell 7: `$PROFILE.CurrentUserAllHosts` de cada um;
- bash: `~/.bashrc`;
- zsh: `$ZDOTDIR/.zshrc`, senao `~/.zshrc`;
- fish: `~/.config/fish/conf.d/agsync.fish`.

## schedule

- Windows: tarefa `agsync-check` do Agendador de Tarefas do usuario, sem admin, rodando `conhost.exe --headless "<agsync.exe>" check` para nao abrir janela.
- Linux: timer `agsync-check` do systemd de usuario quando `systemctl --user` funciona; senao, uma linha no crontab que roda um script em `<dados>` com o PATH congelado.
- `status` sai com codigo `1` quando o agendamento falta ou aponta para outro binario.

## doctor

Confere, nesta ordem: ambiente (WSL), `git`, versao do agsync, agentes detectados, permissao do `secrets.env` e da pasta de backups, agendamento, `machine.toml`, acesso ao repo de config e, para cada item ativo, `requires`, segredos e repos de fonte. Termina com `tudo certo` ou `N problema(s)` e sai com `1` se houver problema.

## uninstall

- `--yes`: desinstala sem perguntar; obrigatorio sem terminal.
- `--purge`: apaga tambem as pastas de config e de dados: `secrets.env`, cache e todos os backups.

Veja [Desinstalar](../guias/desinstalar.md).
