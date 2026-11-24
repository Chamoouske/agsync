# Ajuda da CLI (gerada)

Saida de `agsync <comando> --help`, gerada por `docs/gen-cli-help.sh`. Nao edite a mao: rode o script depois de mudar a CLI. O CI falha se este arquivo estiver desatualizado.

O texto de ajuda da CLI e em ingles; a explicacao de cada comando esta em [Comandos](cli.md).

## `agsync`

```text
Sync MCP servers and skills from a git config repo into coding agents

Usage: agsync <COMMAND>

Commands:
  init            Set up this machine: machine.toml, repo access, schedule, shell hook, first apply
  status          Show applied and remote commits, pending changes, drift, conflicts and skips
  diff            Show what the next apply would change, per agent
  apply           Apply the config repo to every agent found on this machine
  rollback        Apply again the commit applied before the current one, through the normal plan
  check           Read the remote only; record pending changes and notify once. Run by the scheduler
  hook            Print the pending line; no network. Started by the shell profile
  doctor          Check git, the config repo, agents, requires, secrets and source repos
  schedule        Register, remove or show the periodic `agsync check`
  migrate-legacy  Back up and remove what APM, 1MCP and the agent-config scripts left here
  uninstall       Remove every agsync entry from the agents, the schedule, the shell hook and the state
  update          Replace this binary with the latest release (installed by Scoop: tells to run `scoop update agsync`)
  secret          Manage secrets.env, the local file for `${VAR}` values
  help            Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## `agsync init`

```text
Set up this machine: machine.toml, repo access, schedule, shell hook, first apply

Usage: agsync init [OPTIONS] [URL]

Arguments:
  [URL]  Git url of the config repo; asked on the terminal when omitted

Options:
      --ref <GIT_REF>       Branch, tag or full commit SHA to follow (default: main)
      --profile <PROFILES>  Active profile; repeat for several. Asked on the terminal when omitted
      --yes                 Install the shell hook and apply without asking
      --silent-update       Apply pending updates without asking, as `agsync apply --yes` does; the shell hook question stays. What was applied, skipped or failed is still printed. `silentUpdate: true` in agsync.yaml does the same
  -h, --help                Print help
```

## `agsync status`

```text
Show applied and remote commits, pending changes, drift, conflicts and skips

Usage: agsync status

Options:
  -h, --help  Print help
```

## `agsync diff`

```text
Show what the next apply would change, per agent

Usage: agsync diff

Options:
  -h, --help  Print help
```

## `agsync apply`

```text
Apply the config repo to every agent found on this machine

Usage: agsync apply [OPTIONS]

Options:
      --yes    Apply without asking; required when stdin is not a terminal
      --adopt  Replace manual installs (MCP, skill, plugin) whose name, kind and agent match an item of the config repo with its definition, and track them from now on
  -h, --help   Print help
```

## `agsync rollback`

```text
Apply again the commit applied before the current one, through the normal plan

Usage: agsync rollback [OPTIONS]

Options:
      --yes   Apply without asking; required when stdin is not a terminal
  -h, --help  Print help
```

## `agsync check`

```text
Read the remote only; record pending changes and notify once. Run by the scheduler

Usage: agsync check

Options:
  -h, --help  Print help
```

## `agsync hook`

```text
Print the pending line; no network. Started by the shell profile

Usage: agsync hook [COMMAND]

Commands:
  install  Add `agsync hook` to the profile of each shell on this machine
  remove   Remove the agsync block from every shell profile
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## `agsync hook install`

```text
Add `agsync hook` to the profile of each shell on this machine

Usage: agsync hook install [OPTIONS]

Options:
      --yes   Install without asking; required when stdin is not a terminal
  -h, --help  Print help
```

## `agsync hook remove`

```text
Remove the agsync block from every shell profile

Usage: agsync hook remove

Options:
  -h, --help  Print help
```

## `agsync doctor`

```text
Check git, the config repo, agents, requires, secrets and source repos

Usage: agsync doctor

Options:
  -h, --help  Print help
```

## `agsync schedule`

```text
Register, remove or show the periodic `agsync check`

Usage: agsync schedule <COMMAND>

Commands:
  install  Run `agsync check` every `check_interval_minutes` (Task Scheduler, systemd or cron)
  remove   Delete the periodic check
  status   Show whether the check is registered and runs this binary
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## `agsync schedule install`

```text
Run `agsync check` every `check_interval_minutes` (Task Scheduler, systemd or cron)

Usage: agsync schedule install

Options:
  -h, --help  Print help
```

## `agsync schedule remove`

```text
Delete the periodic check

Usage: agsync schedule remove

Options:
  -h, --help  Print help
```

## `agsync schedule status`

```text
Show whether the check is registered and runs this binary

Usage: agsync schedule status

Options:
  -h, --help  Print help
```

## `agsync migrate-legacy`

```text
Back up and remove what APM, 1MCP and the agent-config scripts left here

Usage: agsync migrate-legacy [OPTIONS]

Options:
      --yes   Remove without asking; required when stdin is not a terminal
  -h, --help  Print help
```

## `agsync uninstall`

```text
Remove every agsync entry from the agents, the schedule, the shell hook and the state

Usage: agsync uninstall [OPTIONS]

Options:
      --yes    Uninstall without asking; required when stdin is not a terminal
      --purge  Also delete the config and data dirs: secrets.env, cache and every backup
  -h, --help   Print help
```

## `agsync update`

```text
Replace this binary with the latest release (installed by Scoop: tells to run `scoop update agsync`)

Usage: agsync update [OPTIONS]

Options:
      --yes   Update without asking; required when stdin is not a terminal
  -h, --help  Print help
```

## `agsync secret`

```text
Manage secrets.env, the local file for `${VAR}` values

Usage: agsync secret <COMMAND>

Commands:
  set   Store a value; read without echo from the terminal, or from stdin
  list  Show the names defined in secrets.env (never the values)
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## `agsync secret set`

```text
Store a value; read without echo from the terminal, or from stdin

Usage: agsync secret set <NAME>

Arguments:
  <NAME>  Variable name, as used in `${NAME}`

Options:
  -h, --help  Print help
```

## `agsync secret list`

```text
Show the names defined in secrets.env (never the values)

Usage: agsync secret list

Options:
  -h, --help  Print help
```
