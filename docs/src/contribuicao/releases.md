# Releases

Cada push em `master` (fora `*.md`, `docs/`, `LICENSE*`, `packaging/` e o workflow `docs`) roda o workflow `release`. Esses pushes vem da promocao diaria de `stage` ou de um hotfix (veja [Branches e CI](branches-e-ci.md)).

## Versao

A versao sai dos Conventional Commits desde a ultima tag `v*` (`scripts/next-version.sh`):

- `tipo!:` no titulo ou `BREAKING CHANGE:` no corpo: major;
- algum `feat:`: minor;
- qualquer outro caso: patch.

Sem tag nenhuma, vale a versao do `Cargo.toml`.

## Passos

1. `ci` e os builds de Windows (`x86_64-pc-windows-msvc`) e Linux (`x86_64-unknown-linux-musl`), com a versao gravada no `Cargo.toml` antes de compilar.
2. O job `publish` gera o `SHA256SUMS` dos binarios e publica a release `v<versao>` como `latest` (`make_latest`), com os binarios e o `SHA256SUMS`, e confere que `releases/latest` e essa tag. A release nao depende dos manifests.
3. O job `manifests` baixa o `SHA256SUMS` da release e roda `scripts/update-manifests.sh <versao> dist/SHA256SUMS`, que reescreve versao, URL e hash em `packaging/scoop/agsync.json` e nos tres arquivos de `packaging/winget` (o hash do WinGet em maiusculas, o `autoupdate` do Scoop intacto).
4. Antes de publicar qualquer manifest, `scripts/check-manifests.sh dist/SHA256SUMS` confere que os quatro arquivos tem a mesma versao, o `PackageIdentifier` `Chamoouske.agsync`, as URLs da tag `v<versao>` e o hash do `SHA256SUMS`; `scripts/check-manifests.sh --remote` baixa o binario publicado e compara o hash; `scripts/validate-manifest-schemas.sh` valida contra o schema do Scoop e os schemas 1.6.0 do WinGet.
5. Validados, o `agsync.json` do Scoop e anexado a release (`gh release upload --clobber`), e os manifests vao para a branch `release/manifests-v<versao>` com um PR para `master`, com auto-merge (squash, apagando a branch): o GitHub faz o merge quando os checks obrigatorios passarem. Se nao conseguir ligar o auto-merge, o job deixa um aviso e o PR fica aberto para merge na mao. O merge desse PR nao dispara outra release, porque `packaging/` esta no `paths-ignore`, mas dispara o `sync-stage`, que leva os manifests novos para `stage`.
6. O job `winget` esta desligado: o agsync nao e distribuido pelo WinGet. Ele so roda com a variavel de repositorio `WINGET_SUBMIT` igual a `true` (Settings > Secrets and variables > Actions > Variables) e, nesse caso, envia os arquivos de `packaging/winget` para `microsoft/winget-pkgs` com `wingetcreate submit`; se a versao ja existe la, nao faz nada. Os arquivos de `packaging/winget` continuam sendo atualizados e validados a cada release, para o job poder ser religado sem retrabalho.

## Falhas nos manifests

Os jobs `manifests` e `winget` rodam depois da release, entao uma falha neles nao desfaz nem bloqueia a release. O job que falhou fica vermelho no run e deixa um erro `release v<versao> is published, but ...` dizendo o que nao foi atualizado. Corrija a causa e use "Re-run failed jobs" no run; os dois jobs podem rodar de novo (o anexo usa `--clobber`, a branch e reenviada e o PR existente e reaproveitado). Ate isso, `releases/latest/download/agsync.json` nao existe na release nova.

O workflow `ci` tambem tem o job `manifests`: passa o `actionlint` nos workflows, roda `scripts/test-manifests.sh` (manifest desatualizado, URL de outra versao, hash divergente do `SHA256SUMS`, do outro manifest ou do artefato publicado), confere os manifests de `master` contra o binario publicado e valida os schemas.

## Configuracao externa

- Secret `RELEASE_PR_TOKEN`: token (fine-grained, com Contents e Pull requests de escrita neste repo) usado para abrir o PR de manifests e os PRs dos workflows `promote` e `sync-stage`. O `promote` falha sem ele, porque um merge feito com o `GITHUB_TOKEN` nao dispara o workflow `release`. Sem ele, o job `manifests` e o `sync-stage` usam o `GITHUB_TOKEN`, e o GitHub nao roda o `ci` em PR aberto por esse token, entao os checks obrigatorios ficam pendentes; feche e reabra o PR ou faca um push nele para rodar o CI, senao o auto-merge nunca acontece. Sem o secret, abrir o PR tambem exige Settings > Actions > General > "Allow GitHub Actions to create and approve pull requests"; se nem isso estiver ligado, o job deixa um aviso com o link para abrir o PR na mao.
- Settings > General > "Allow auto-merge" ligado. Sem isso os PRs de manifests, `promote` e `sync-stage` sao abertos, mas o merge fica manual, e os jobs `promote` e `sync-stage` falham para avisar.
- Branch padrao do repo: `stage`, para os PRs ja abrirem com o alvo certo. O agendamento do `promote` roda a partir do arquivo de workflow da branch padrao.
- Secret `WINGET_TOKEN` (so usado com `WINGET_SUBMIT=true`): token classic com o escopo `public_repo` de uma conta que pode fazer fork de `microsoft/winget-pkgs`; o `wingetcreate` abre o PR a partir desse fork. Sem o secret, o job `winget` deixa um aviso e nao envia nada; envie `packaging/winget` na mao com `wingetcreate submit`. O PR em `microsoft/winget-pkgs` ainda passa pela validacao e revisao de la; `winget install` so encontra a versao nova depois do merge.
- GitHub Pages com a fonte "GitHub Actions" (Settings > Pages), para o workflow `docs` publicar esta documentacao. O environment `github-pages` (Settings > Environments) precisa aceitar deploy da branch `master`.
