# Primeiro uso

Você precisa de um repo git com um `agsync.yaml` na raiz. Se ainda não tem um, veja [Montar o repo de config](../guias/repo-de-config.md).

## init

```sh
agsync init https://github.com/<org>/agent-config --profile work
```

O `init`:

1. grava o `machine.toml`;
2. testa o acesso ao repo;
3. registra o `agsync check` periódico;
4. oferece o hook em cada shell;
5. roda o primeiro `apply`.

Cada passo reporta a própria falha, e os passos seguintes ainda rodam quando fazem sentido.

Sem a URL ou sem `--profile`, o `init` pergunta no terminal só o que faltou; o que foi passado não é perguntado de novo:

```text
$ agsync init
URL do repo de config: https://github.com/<org>/agent-config
Perfis desta maquina (separados por virgula): work, personal
```

Sem resposta (stdin fechado, sem terminal), o `init` para antes de gravar o `machine.toml`.

Opções:

- `--ref <branch|tag|sha>`: o padrão é `main`; nunca é perguntado.
- `--profile` pode repetir; sem ele, os perfis são perguntados.
- `--yes`: instala o hook e aplica sem perguntar.
- `--silent-update`: aplica sem perguntar, como `agsync apply --yes`, também sem terminal. O hook de shell continua perguntando. O resultado continua impresso, e uma falha faz o `init` sair com erro. `silentUpdate: true` no `agsync.yaml` tem o mesmo efeito para toda máquina que seguir o repo.
- Exceção: com build de MCP vindo de um import, `--silent-update` e `silentUpdate` não dispensam a confirmação. Só `--yes` explícito aplica sem perguntar.

Repo privado: o `git` precisa de credencial própria, como chave SSH ou credential helper. O agsync nunca pede senha.

## Conferir

```sh
agsync status
agsync doctor
```

- `status` mostra o commit aplicado e o remoto, mudanças pendentes, desvios, conflitos e itens pulados.
- `doctor` confere git, repo, agentes, `requires`, segredos, permissões, agendamento e repos de fonte.

MCPs só valem depois de reiniciar o agente.

## Dia a dia

1. Alguém muda o `agsync.yaml` no repo de config.
2. O `agsync check` periódico vê a mudança, grava `pending.json` e notifica uma vez.
3. Ao abrir um shell, o hook mostra a linha de pendência.
4. Você confere com `agsync diff` e aplica com `agsync apply`.

Veja [Ciclo check, hook e apply](../explicacao/ciclo.md).
