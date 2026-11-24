# Solucao de problemas

Comece sempre por:

```sh
agsync doctor
agsync status
```

O `doctor` aponta a maior parte dos problemas de ambiente; o `status` mostra itens pulados com o motivo.

## Microsoft Defender e SmartScreen

O binario do Windows nao tem assinatura de codigo e chama o PowerShell por dentro (por exemplo, para achar os perfis do PowerShell). Por isso o classificador de aprendizado de maquina do Microsoft Defender pode por o `agsync.exe` em quarentena (por exemplo, como `Trojan:Win32/Bearfoos.A!ml`), ou o SmartScreen pode avisar sobre um aplicativo desconhecido. E um falso positivo conhecido, mas confira antes de liberar.

Sinais: o `install.ps1` termina com `... sumiu ou nao executa depois da instalacao`, o `agsync update` volta para o binario anterior, ou o `agsync.exe` some da pasta.

1. Confira o SHA256 do arquivo contra o `SHA256SUMS` da release:

   ```powershell
   (Get-FileHash "$env:LOCALAPPDATA\agsync\bin\agsync.exe" -Algorithm SHA256).Hash.ToLower()
   ```

   Compare com a linha de `agsync-x86_64-pc-windows-msvc.exe` no `SHA256SUMS` da [release](https://github.com/Chamoouske/agsync/releases). Se o hash for diferente, nao use o arquivo. O `install.ps1` ja faz essa conferencia antes de copiar o binario.

2. Para restaurar da quarentena: Seguranca do Windows > Protecao contra virus e ameacas > Historico de protecao, escolha a deteccao do `agsync.exe` e use Acoes > Restaurar. Depois rode o instalador de novo se o arquivo tiver sumido.

3. Se o Defender voltar a bloquear, exclua so o arquivo, nunca a pasta inteira, em um PowerShell como administrador:

   ```powershell
   Add-MpPreference -ExclusionPath "$env:LOCALAPPDATA\agsync\bin\agsync.exe"
   ```

   Risco: o Defender deixa de examinar esse arquivo. Faca isso so depois de conferir o SHA256, e remova a exclusao com `Remove-MpPreference -ExclusionPath` se trocar de binario por um que voce nao conferiu.

4. Para ajudar a corrigir a deteccao, envie o arquivo como falso positivo em <https://www.microsoft.com/wdsi/filesubmission>.

Nao rode o instalador com `-ExecutionPolicy Bypass`: o Defender costuma marcar justamente essa linha.

## Repo inacessivel ou credencial

- O agsync usa o `git` do sistema e nunca pede senha. Um repo privado precisa de chave SSH ou credential helper configurado para o seu usuario. Teste com `git ls-remote <url>`.
- Erros de TLS/certificado ou de autenticacao recebem uma dica no `doctor`.
- URL com token embutido (`https://token@host/...`) e recusada de proposito; use o credential helper.
- O `check` com remoto inacessivel so registra em `<dados>/logs/check.log`, sem notificar.

## Nada acontece depois de mudar o repo

- O `check` roda a cada `check_interval_minutes` (padrao 15). Rode `agsync status` para consultar o remoto na hora.
- Confira o agendamento com `agsync schedule status`. Se ele aponta para outro binario (por exemplo, depois de mover o agsync), rode `agsync schedule install`.
- Veja `<dados>/logs/check.log`.
- O `check` so avisa; aplique com `agsync apply`.

## MCP nao aparece no agente

- Reinicie o agente: MCPs so valem depois disso. No OpenCode, `opencode reload`; no `agy`, `/mcp` e `/skills reload`.
- O agente foi detectado? O executavel precisa estar no PATH do processo que roda o agsync. Veja `agsync doctor`.
- O item esta restrito por `profiles` ou `agents`? Veja `agsync status`.
- Faltou um segredo ou um programa de `requires`? O item e pulado e o motivo aparece em `status`.

## Item marcado como conflito

Existe uma instalacao manual com o mesmo nome, tipo e agente. Veja [Assumir instalacoes manuais](guias/adotar.md).

## Skill restrita aparece no OpenCode

O OpenCode tambem le `~/.claude/skills` e `~/.agents/skills`. Uma skill com `agents: [claude]` fica visivel nele por isso; o `status` avisa.

## Erro no agsync.yaml

A mensagem traz a linha: `agsync.yaml linha N: ...`. Os erros mais comuns:

- `defina exatamente um entre command e url`;
- `perfil 'x' nao existe em profiles`;
- `ref '...' parece SHA abreviado; use o SHA completo`;
- `... com credenciais embutidas; use o credential helper do git`;
- `skill '...' sem SKILL.md no repo`.

Veja [agsync.yaml](referencia/agsync-yaml.md).

## Comando parado esperando confirmacao, ou falhando sem terminal

Sem terminal, `apply`, `rollback`, `migrate-legacy`, `uninstall`, `update` e `hook install` precisam de `--yes`. Em scripts, passe `--yes` explicitamente.

## Outro agsync em execucao

Comandos que escrevem esperam o lock em `<dados>/agsync.lock` e dizem isso na stderr. O `check` desiste em silencio se o lock estiver ocupado.

## Rollback falha com "sem commit anterior"

O `rollback` precisa de dois applies de commits diferentes. Veja [Voltar para o commit anterior](guias/rollback.md).

## Rollback e o check continua acusando pendencia

E esperado: o remoto continua no commit novo. Corrija o repo e rode `agsync apply`.

## Toast do Windows mostra "Windows PowerShell"

E esperado: o agsync usa a identidade de app do Windows PowerShell para mostrar o toast sem registrar nada. O titulo do toast e "agsync".

## WSL

Agentes instalados so no Windows nao contam no WSL; o `doctor` diz quando isso acontece. Veja [Usar no WSL](guias/wsl.md).
