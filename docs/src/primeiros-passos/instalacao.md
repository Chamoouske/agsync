# Instalacao

Requisito: `git` no PATH. O agsync usa o `git` do sistema para todo acesso a repos.

Ha binarios para Windows x86_64 e Linux x86_64 (estatico, musl). Os scripts de instalacao baixam o binario do GitHub Releases e conferem o SHA256 com o `SHA256SUMS` da release. Os binarios nao tem assinatura de codigo.

## Windows (PowerShell)

Sem admin. Instala em `%LOCALAPPDATA%\agsync\bin` e poe essa pasta no PATH do usuario:

```powershell
irm https://raw.githubusercontent.com/Chamoouske/agsync/master/install.ps1 | iex
```

Uma versao especifica:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/Chamoouske/agsync/master/install.ps1))) -Version 0.14.2
```

Nao use `-ExecutionPolicy Bypass`. O `irm ... | iex` roda sem mudar a politica de execucao, e o Defender costuma marcar justamente essa linha como suspeita.

Depois de copiar o binario, o script executa `agsync.exe --version`. Se o arquivo sumiu ou nao executa, o script para com um erro que aponta o guia do Defender: veja [Microsoft Defender e SmartScreen](../solucao-de-problemas.md#microsoft-defender-e-smartscreen).

Se o PATH do usuario mudou, abra um terminal novo.

## Scoop

Pelo manifest anexado a ultima release:

```powershell
scoop install https://github.com/Chamoouske/agsync/releases/latest/download/agsync.json
```

O manifest de `packaging/scoop` nao esta no formato de bucket, entao o Scoop instala direto pela URL. O manifest de `master` (`https://raw.githubusercontent.com/Chamoouske/agsync/master/packaging/scoop/agsync.json`) tambem funciona, mas so aponta para a versao nova depois do merge do PR de manifests (veja [Releases](../contribuicao/releases.md)).

Instalado pelo Scoop, atualize com `scoop update agsync`; o `agsync update` so avisa isso.

## Linux

x86_64. Instala em `~/.local/bin`. Se essa pasta nao esta no PATH, acrescenta `export PATH="$HOME/.local/bin:$PATH"` ao `~/.profile` e aos `~/.bashrc`/`~/.zshrc` que existirem:

```sh
curl -fsSL https://raw.githubusercontent.com/Chamoouske/agsync/master/install.sh | sh
```

Uma versao especifica:

```sh
curl -fsSL https://raw.githubusercontent.com/Chamoouske/agsync/master/install.sh | sh -s -- --version 0.14.2
```

O script baixa com `curl` ou, sem ele, com `wget`. Em outra arquitetura ele para com erro.

## Cargo

Com Rust instalado, a partir de uma tag:

```sh
cargo install --git https://github.com/Chamoouske/agsync --tag v0.14.2
```

## Mirror das releases

Os dois scripts e o `agsync update` aceitam `AGSYNC_INSTALL_BASE_URL` no lugar de `https://github.com/Chamoouske/agsync/releases`. O servidor precisa servir `<base>/latest/download/<asset>` e `<base>/download/v<versao>/<asset>`, com o `SHA256SUMS` ao lado. Veja [Variaveis de ambiente](../referencia/variaveis-de-ambiente.md).

## Proximo passo

```sh
agsync init <url do repo de config> --profile <perfil>
```

Veja [Primeiro uso](primeiro-uso.md).
