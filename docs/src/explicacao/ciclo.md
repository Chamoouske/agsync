# Ciclo check, hook e apply

Tres comandos dividem o trabalho de manter a maquina em dia sem aplicar nada sozinho.

```text
 agendador (a cada check_interval_minutes)
      |
      v
 agsync check --(rede: ls-remote, snapshot)--> pending.json, update.json, notificacao
                                                   |
 shell abre                                        |
      |                                            v
      v                                    linha de pendencia
 agsync hook --(sem rede: le arquivos)----> no terminal
      |
      +-- check atrasado? dispara `agsync check` em segundo plano

 voce: agsync diff / agsync apply  --> agentes, state.json; limpa pending.json
```

## check

Roda pelo agendador, a cada `check_interval_minutes` (padrao 15).

1. Se outra execucao segura o lock, sai na hora.
2. Procura versao nova do agsync pelas tags do repo do agsync; achando, grava `update.json` e notifica uma vez por versao.
3. Resolve a `ref` do repo de config com `git ls-remote`. Remoto inacessivel: registra no log e para, sem erro.
4. Se o commit e as fontes moveis sao os mesmos do ultimo plano (`watch.json`), para.
5. Senao, planeja como o `diff` faria e grava em `pending.json` o que o `apply` mudaria, os builds e as instalacoes manuais em conflito. Um `agsync.yaml` invalido tambem vira pendencia.
6. Notifica uma vez por novidade: a mesma pendencia nao gera outra notificacao.

Cada passo vira uma linha em `<dados>/logs/check.log`.

## hook

Roda quando o shell abre. Le `pending.json` e `update.json` e imprime uma linha para cada. Nunca acessa a rede.

Se nenhum `check` alcancou o remoto ha mais que o dobro do intervalo, e o proprio hook nao disparou um ha mais que um intervalo, ele inicia `agsync check` em segundo plano, sem janela. Isso cobre maquinas sem agendador funcionando, como o WSL sem systemd e sem cron.

Os outros comandos (menos `check`, `apply` e `update`) imprimem as mesmas linhas na stderr antes de rodar.

## apply

So o `apply` (e o `rollback`) muda os agentes. Ele refaz o plano a partir do remoto atual, mostra o diff e os builds, pede confirmacao e escreve. Se algum item falha, os outros seguem e o comando sai com erro. Ao terminar sem falhas, limpa `pending.json` e atualiza `watch.json`.

## Notificacoes

- Windows: toast pelo WinRT, usando a identidade de app do Windows PowerShell, que ja vem com o Windows; nada e registrado. O toast tem o titulo "agsync" e o Windows mostra o Windows PowerShell como remetente.
- Linux: `notify-send`, so com sessao grafica (`DISPLAY` ou `WAYLAND_DISPLAY`).
- WSL: sem notificacao de desktop; as novidades so aparecem no terminal.
