# Solução de problemas

Comece sempre por:

```sh
agsync doctor
agsync status
```

O `doctor` aponta a maior parte dos problemas de ambiente; o `status` mostra itens pulados com o motivo.

## Microsoft Defender e SmartScreen

O binário do Windows não tem assinatura de código e chama o PowerShell por dentro (por exemplo, para achar os perfis do PowerShell). Por isso o classificador de aprendizado de máquina do Microsoft Defender pode por o `agsync.exe` em quarentena (por exemplo, como `Trojan:Win32/Bearfoos.A!ml`), ou o SmartScreen pode avisar sobre um aplicativo desconhecido. É um falso positivo conhecido e ainda acontece nas versões atuais, mas confira antes de liberar.

Sinais: o `install.ps1` termina com `... sumiu ou nao executa depois da instalacao`, o `agsync update` volta para o binário anterior, ou o `agsync.exe` some da pasta.

1. Confira o SHA256 do arquivo contra o `SHA256SUMS` da release:

   ```powershell
   (Get-FileHash "$env:LOCALAPPDATA\agsync\bin\agsync.exe" -Algorithm SHA256).Hash.ToLower()
   ```

   Compare com a linha de `agsync-x86_64-pc-windows-msvc.exe` no `SHA256SUMS` da [release](https://github.com/Chamoouske/agsync/releases). Se o hash for diferente, não use o arquivo. O `install.ps1` já faz essa conferência antes de copiar o binário.

2. Para restaurar da quarentena: Segurança do Windows > Proteção contra vírus e ameaças > Histórico de proteção, escolha a detecção do `agsync.exe` e use Ações > Restaurar. Depois rode o instalador de novo se o arquivo tiver sumido.

3. Se o Defender voltar a bloquear, exclua só o arquivo, nunca a pasta inteira, em um PowerShell como administrador:

   ```powershell
   Add-MpPreference -ExclusionPath "$env:LOCALAPPDATA\agsync\bin\agsync.exe"
   ```

   Risco: o Defender deixa de examinar esse arquivo. Faça isso só depois de conferir o SHA256, e remova a exclusão com `Remove-MpPreference -ExclusionPath` se trocar de binário por um que você não conferiu.

4. Para ajudar a corrigir a detecção, envie o arquivo como falso positivo em <https://www.microsoft.com/wdsi/filesubmission>.

Não rode o instalador com `-ExecutionPolicy Bypass`: o Defender costuma marcar justamente essa linha.

## Repo inacessível ou credencial

- O agsync usa o `git` do sistema e nunca pede senha. Um repo privado precisa de chave SSH ou credential helper configurado para o seu usuário. Teste com `git ls-remote <url>`.
- Erros de TLS/certificado ou de autenticação recebem uma dica no `doctor`.
- URL com token embutido (`https://token@host/...`) é recusada de propósito; use o credential helper.
- O `check` com remoto inacessível só registra em `<dados>/logs/check.log`, sem notificar.

## Nada acontece depois de mudar o repo

- O `check` roda a cada `check_interval_minutes` (padrão 15). Rode `agsync status` para consultar o remoto na hora.
- Confira o agendamento com `agsync schedule status`. Se ele aponta para outro binário (por exemplo, depois de mover o agsync), rode `agsync schedule install`.
- Veja `<dados>/logs/check.log`.
- O `check` só avisa; aplique com `agsync apply`.

## MCP não aparece no agente

- Reinicie o agente: MCPs só valem depois disso. No OpenCode, `opencode reload`; no `agy`, `/mcp` e `/skills reload`.
- O agente foi detectado? O executável precisa estar no PATH do processo que roda o agsync. Veja `agsync doctor`.
- O item está restrito por `profiles` ou `agents`? Veja `agsync status`.
- Faltou um segredo ou um programa de `requires`? O item é pulado e o motivo aparece em `status`.

## Item marcado como conflito

Existe uma instalação manual com o mesmo nome, tipo e agente. Veja [Assumir instalações manuais](guias/adotar.md).

## Skill restrita aparece no OpenCode

O OpenCode também lê `~/.claude/skills` e `~/.agents/skills`. Uma skill com `agents: [claude]` fica visível nele por isso; o `status` avisa.

## Erro no agsync.yaml

A mensagem traz a linha: `agsync.yaml linha N: ...`. Os erros mais comuns:

- `defina exatamente um entre command e url`;
- `perfil 'x' nao existe em profiles`;
- `ref '...' parece SHA abreviado; use o SHA completo`;
- `... com credenciais embutidas; use o credential helper do git`;
- `skill '...' sem SKILL.md no repo`.

Veja [agsync.yaml](referencia/agsync-yaml.md).

## Comando parado esperando confirmação, ou falhando sem terminal

Sem terminal, `apply`, `rollback`, `migrate-legacy`, `uninstall`, `update` e `hook install` precisam de `--yes`. Em scripts, passe `--yes` explicitamente.

## Outro agsync em execução

Comandos que escrevem esperam o lock em `<dados>/agsync.lock` e dizem isso na stderr. O `check` desiste em silêncio se o lock estiver ocupado.

## Rollback falha com "sem commit anterior"

O `rollback` precisa de dois applies de commits diferentes. Veja [Voltar para o commit anterior](guias/rollback.md).

## Rollback e o check continua acusando pendência

É esperado: o remoto continua no commit novo. Corrija o repo e rode `agsync apply`.

## Toast do Windows mostra "Windows PowerShell"

É esperado: o agsync usa a identidade de app do Windows PowerShell para mostrar o toast sem registrar nada. O título do toast e "agsync".

## WSL

Agentes instalados só no Windows não contam no WSL; o `doctor` diz quando isso acontece. Veja [Usar no WSL](guias/wsl.md).
