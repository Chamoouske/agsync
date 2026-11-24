# Primeiro uso

Voce precisa de um repo git com um `agsync.yaml` na raiz. Se ainda nao tem um, veja [Montar o repo de config](../guias/repo-de-config.md).

## init

```sh
agsync init https://github.com/<org>/agent-config --profile work
```

O `init`:

1. grava o `machine.toml`;
2. testa o acesso ao repo;
3. registra o `agsync check` periodico;
4. oferece o hook em cada shell;
5. roda o primeiro `apply`.

Cada passo reporta a propria falha, e os passos seguintes ainda rodam quando fazem sentido.

Sem a URL ou sem `--profile`, o `init` pergunta no terminal so o que faltou; o que foi passado nao e perguntado de novo:

```text
$ agsync init
URL do repo de config: https://github.com/<org>/agent-config
Perfis desta maquina (separados por virgula): work, personal
```

Sem resposta (stdin fechado, sem terminal), o `init` para antes de gravar o `machine.toml`.

Opcoes:

- `--ref <branch|tag|sha>`: o padrao e `main`; nunca e perguntado.
- `--profile` pode repetir; sem ele, os perfis sao perguntados.
- `--yes`: instala o hook e aplica sem perguntar.
- `--silent-update`: aplica sem perguntar, como `agsync apply --yes`, tambem sem terminal. O hook de shell continua perguntando. O resultado continua impresso, e uma falha faz o `init` sair com erro. `silentUpdate: true` no `agsync.yaml` tem o mesmo efeito para toda maquina que seguir o repo.
- Excecao: com build de MCP vindo de um import, `--silent-update` e `silentUpdate` nao dispensam a confirmacao. So `--yes` explicito aplica sem perguntar.

Repo privado: o `git` precisa de credencial propria, como chave SSH ou credential helper. O agsync nunca pede senha.

## Conferir

```sh
agsync status
agsync doctor
```

- `status` mostra o commit aplicado e o remoto, mudancas pendentes, desvios, conflitos e itens pulados.
- `doctor` confere git, repo, agentes, `requires`, segredos, permissoes, agendamento e repos de fonte.

MCPs so valem depois de reiniciar o agente.

## Dia a dia

1. Alguem muda o `agsync.yaml` no repo de config.
2. O `agsync check` periodico ve a mudanca, grava `pending.json` e notifica uma vez.
3. Ao abrir um shell, o hook mostra a linha de pendencia.
4. Voce confere com `agsync diff` e aplica com `agsync apply`.

Veja [Ciclo check, hook e apply](../explicacao/ciclo.md).
