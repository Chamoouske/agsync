# Ciclo check, hook e apply

Três comandos dividem o trabalho de manter a máquina em dia sem aplicar nada sozinho.

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

Roda pelo agendador, a cada `check_interval_minutes` (padrão 15).

1. Se outra execução segura o lock, sai na hora.
2. Procura versão nova do agsync pelas tags do repo do agsync; achando, grava `update.json` e notifica uma vez por versão.
3. Resolve a `ref` do repo de config com `git ls-remote`. Remoto inacessível: registra no log e para, sem erro.
4. Se o commit e as fontes móveis são os mesmos do último plano (`watch.json`), para.
5. Senão, planeja como o `diff` faria e grava em `pending.json` o que o `apply` mudaria, os builds e as instalações manuais em conflito. Um `agsync.yaml` inválido também vira pendência.
6. Notifica uma vez por novidade: a mesma pendência não gera outra notificação.

Cada passo vira uma linha em `<dados>/logs/check.log`.

## hook

Roda quando o shell abre. Lê `pending.json` e `update.json` e imprime uma linha para cada. Nunca acessa a rede.

Se nenhum `check` alcançou o remoto há mais que o dobro do intervalo, e o próprio hook não disparou um há mais que um intervalo, ele inicia `agsync check` em segundo plano, sem janela. Isso cobre máquinas sem agendador funcionando, como o WSL sem systemd e sem cron.

Os outros comandos (menos `check`, `apply` e `update`) imprimem as mesmas linhas na stderr antes de rodar.

## apply

Só o `apply` (e o `rollback`) muda os agentes. Ele refaz o plano a partir do remoto atual, mostra o diff e os builds, pede confirmação e escreve. Se algum item falha, os outros seguem e o comando sai com erro. Ao terminar sem falhas, limpa `pending.json` e atualiza `watch.json`.

## Notificações

- Windows: toast pelo WinRT, usando a identidade de app do Windows PowerShell, que já vem com o Windows; nada é registrado. O toast tem o título "agsync" e o Windows mostra o Windows PowerShell como remetente.
- Linux: `notify-send`, só com sessão gráfica (`DISPLAY` ou `WAYLAND_DISPLAY`).
- WSL: sem notificação de desktop; as novidades só aparecem no terminal.
