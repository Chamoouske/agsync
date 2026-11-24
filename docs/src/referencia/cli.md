# Comandos

Visão de cada comando. A saída exata de `--help` de cada um está em [Ajuda da CLI (gerada)](cli-help.md).

| comando | o que faz |
|---|---|
| `agsync init [<url>] [--ref R] [--profile P]... [--yes] [--silent-update]` | configura a máquina: `machine.toml`, acesso ao repo, agendamento, hook de shell e primeiro `apply`; pergunta a URL e os perfis que faltarem |
| `agsync status` | commit aplicado e remoto, mudanças pendentes, desvios, conflitos e itens pulados |
| `agsync diff` | o que o próximo `apply` muda, por agente |
| `agsync apply [--yes] [--adopt]` | mostra o diff e os builds, pede confirmação e aplica |
| `agsync rollback [--yes]` | reaplica o commit aplicado antes do atual, pelo mesmo fluxo de plano e apply |
| `agsync check` | só lê o remoto: grava `pending.json` e notifica uma vez por novidade; antes, procura versão nova do agsync; roda pelo agendador |
| `agsync hook` | imprime a linha de pendência e a de versão nova, sem rede; roda pelo perfil do shell |
| `agsync hook install [--yes]` | põe o hook nos perfis de PowerShell, pwsh, bash, zsh e fish |
| `agsync hook remove` | tira o bloco do agsync de todos os perfis de shell |
| `agsync schedule install` | registra o `check` periódico a cada `check_interval_minutes` |
| `agsync schedule remove` | apaga o `check` periódico |
| `agsync schedule status` | diz se o `check` está registrado e roda este binário |
| `agsync secret set NOME` | grava um valor no `secrets.env`, lido sem eco ou da stdin |
| `agsync secret list` | lista os nomes do `secrets.env`, nunca os valores |
| `agsync doctor` | confere git, repo, agentes, `requires`, segredos, permissões, agendamento e repos de fonte |
| `agsync update [--yes]` | troca o binário pela última release; instalado pelo Scoop, só diz para rodar `scoop update agsync` |
| `agsync migrate-legacy [--yes]` | backup e remoção do sistema antigo (APM, 1MCP, agent-config) |
| `agsync uninstall [--yes] [--purge]` | tira dos agentes tudo o que está no state, remove agendamento e hooks e apaga `state.json` e `pending.json` |

`agsync --version` imprime a versão; `agsync help <comando>` e `agsync <comando> --help` imprimem a ajuda.

## Confirmação e terminal

`apply`, `rollback`, `migrate-legacy`, `uninstall`, `update` e `hook install` perguntam antes de mudar algo. Sem terminal (stdin que não é TTY), eles só rodam com `--yes`.

Responder não a confirmação encerra o comando sem mudar nada e com código `0`.

## Linha de pendência

Todo comando, menos `hook`, `check`, `apply` e `update`, imprime antes na stderr a linha de pendência e a de versão nova, se houver. O `hook` imprime as mesmas linhas na stdout.

## init

- `<url>`: URL git do repo de config. Perguntada quando omitida.
- `--ref <branch|tag|sha>`: o que seguir; padrão `main`. SHA precisa ser completo.
- `--profile P`: perfil ativo; repita para vários. Perguntado quando omitido.
- `--yes`: instala o hook e aplica sem perguntar.
- `--silent-update`: aplica sem perguntar, também sem terminal; o hook continua perguntando. Não vale para build de MCP importado.

Passos: `[1/5] machine.toml`, acesso ao repo, agendamento, hook, primeiro `apply`. Com o repo inacessível, o `apply` é pulado com um aviso. Veja [Primeiro uso](../primeiros-passos/primeiro-uso.md).

## apply

- `--yes`: aplica sem perguntar; obrigatório sem terminal.
- `--adopt`: substitui instalações manuais (MCP, skill, plugin) com mesmo nome, tipo e agente de um item do repo pela definição do repo, e passa a gerenciá-las. Veja [Assumir instalações manuais](../guias/adotar.md).

## rollback

Planeja e aplica o commit guardado em `previous` no `state.json`. Falha se não há commit anterior. Veja [Voltar para o commit anterior](../guias/rollback.md).

## check

Não pergunta nada e nunca aplica. Se outra execução segura o lock, sai na hora, em silêncio. Remoto inacessível vai para o log e não é erro. Cada evento vira uma linha em `<dados>/logs/check.log`. Veja [Ciclo check, hook e apply](../explicacao/ciclo.md).

## hook

Sem subcomando: imprime as linhas de pendência e de versão nova e, se nenhum `check` teve sucesso há mais que o dobro do intervalo, dispara um `check` em segundo plano. Nunca acessa a rede.

O bloco instalado nos perfis fica entre as linhas `# >>> agsync hook >>>` e `# <<< agsync hook <<<` e só chama o `agsync hook` se o binário existir. Perfis:

- PowerShell 5.1 e PowerShell 7: `$PROFILE.CurrentUserAllHosts` de cada um;
- bash: `~/.bashrc`;
- zsh: `$ZDOTDIR/.zshrc`, senão `~/.zshrc`;
- fish: `~/.config/fish/conf.d/agsync.fish`.

## schedule

- Windows: tarefa `agsync-check` do Agendador de Tarefas do usuário, sem admin, rodando `conhost.exe --headless "<agsync.exe>" check` para não abrir janela.
- Linux: timer `agsync-check` do systemd de usuário quando `systemctl --user` funciona; senão, uma linha no crontab que roda um script em `<dados>` com o PATH congelado.
- `status` sai com código `1` quando o agendamento falta ou aponta para outro binário.

## doctor

Confere, nesta ordem: ambiente (WSL), `git`, versão do agsync, agentes detectados, permissão do `secrets.env` e da pasta de backups, agendamento, `machine.toml`, acesso ao repo de config e, para cada item ativo, `requires`, segredos e repos de fonte. Termina com `tudo certo` ou `N problema(s)` e sai com `1` se houver problema.

## uninstall

- `--yes`: desinstala sem perguntar; obrigatório sem terminal.
- `--purge`: apaga também as pastas de config e de dados: `secrets.env`, cache e todos os backups.

Veja [Desinstalar](../guias/desinstalar.md).
