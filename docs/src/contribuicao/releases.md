# Releases

Cada push em `master` (fora `*.md`, `docs/`, `LICENSE*`, `packaging/` e o workflow `docs`) roda o workflow `release`. Esses pushes vêm da promoção diária de `stage` ou de um hotfix (veja [Branches e CI](branches-e-ci.md)).

## Versão

A versão sai dos Conventional Commits desde a última tag `v*` (`scripts/next-version.sh`):

- `tipo!:` no título ou `BREAKING CHANGE:` no corpo: major;
- algum `feat:`: minor;
- qualquer outro caso: patch.

Sem tag nenhuma, vale a versão do `Cargo.toml`.

## Passos

1. `ci` e os builds de Windows (`x86_64-pc-windows-msvc`) e Linux (`x86_64-unknown-linux-musl`), com a versão gravada no `Cargo.toml` antes de compilar.
2. O job `publish` gera o `SHA256SUMS` dos binários, escreve as notas da release com o git-cliff (a mesma seção que entra no `CHANGELOG.md`, veja [Changelog](#changelog)) e publica a release `v<versao>` como `latest` (`make_latest`), com os binários e o `SHA256SUMS`, e confere que `releases/latest` é essa tag. A release não depende dos manifests.
3. O job `manifests` baixa o `SHA256SUMS` da release e roda `scripts/update-manifests.sh <versao> dist/SHA256SUMS`, que reescreve versão, URL e hash em `packaging/scoop/agsync.json` e nos três arquivos de `packaging/winget` (o hash do WinGet em maiúsculas, o `autoupdate` do Scoop intacto). No mesmo passo de commit, o git-cliff acrescenta a seção `## [<versao>]` no topo do `CHANGELOG.md`.
4. Antes de publicar qualquer manifest, `scripts/check-manifests.sh dist/SHA256SUMS` confere que os quatro arquivos têm a mesma versão, o `PackageIdentifier` `Chamoouske.agsync`, as URLs da tag `v<versao>` e o hash do `SHA256SUMS`; `scripts/check-manifests.sh --remote` baixa o binário publicado e compara o hash; `scripts/validate-manifest-schemas.sh` valida contra o schema do Scoop e os schemas 1.6.0 do WinGet.
5. Validados, o `agsync.json` do Scoop é anexado à release (`gh release upload --clobber`), e o `CHANGELOG.md` e os manifests vão, num commit `chore(release): changelog and manifests for v<versao>`, para a branch `release/manifests-v<versao>` com um PR para `master`, com auto-merge (squash, apagando a branch): o GitHub faz o merge quando os checks obrigatórios passarem. Se não conseguir ligar o auto-merge, o job deixa um aviso e o PR fica aberto para merge na mão. O merge desse PR não dispara outra release, porque `packaging/` e `**/*.md` estão no `paths-ignore`, mas dispara o `sync-stage`, que leva o `CHANGELOG.md` e os manifests novos para `stage`. A promoção seguinte não vê diferença entre `stage` e `master` por causa disso, então não há laço.
6. O job `winget` está desligado: o agsync não é distribuído pelo WinGet. Ele só roda com a variável de repositório `WINGET_SUBMIT` igual a `true` (Settings > Secrets and variables > Actions > Variables) e, nesse caso, envia os arquivos de `packaging/winget` para `microsoft/winget-pkgs` com `wingetcreate submit`; se a versão já existe lá, não faz nada. Os arquivos de `packaging/winget` continuam sendo atualizados e validados a cada release, para o job poder ser religado sem retrabalho.

## Changelog

O [`CHANGELOG.md`](../changelog.md), na raiz do repo, segue o [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e é gerado pelo [git-cliff](https://git-cliff.org/) (versão fixada em `GIT_CLIFF_VERSION` no workflow `release`) a partir dos Conventional Commits, com a configuração em `cliff.toml`. Não edite o arquivo à mão: o que vale é a mensagem do commit.

- Na release, o git-cliff gera a seção da versão nova com os commits desde a tag anterior e a acrescenta no topo (`--prepend`), sem mexer nas seções antigas. O link de comparação da versão fica logo abaixo da seção.
- O tipo do commit decide a seção: `feat` em Adicionado, `perf` e `refactor` em Alterado, `fix` em Corrigido, `docs` em Documentação, `ci` e `build` em CI, `chore` em Manutenção. Commits com `!` ou `BREAKING CHANGE:` aparecem com **Incompatível:** na frente. Ficam de fora os merges, `test`, `style`, `chore(packaging)` e `chore(release)`.
- A linha do changelog é a descrição do commit, com a primeira letra maiúscula: escreva a descrição pensando em quem lê o changelog.
- As notas da release no GitHub são a mesma seção, gerada pelo mesmo comando.
- O check `conventional commits` (workflow `conventional-commits`, só em PR) recusa PR cujo título ou algum commit (fora merges) não siga Conventional Commits (`scripts/check-conventional.sh`).

Para ver o que entraria na próxima versão, sem criar tag:

```bash
git cliff --unreleased --tag v<proxima-versao> --strip all
```

## Falhas nos manifests

Os jobs `manifests` e `winget` rodam depois da release, então uma falha neles não desfaz nem bloqueia a release. O job que falhou fica vermelho no run e deixa um erro `release v<versao> is published, but ...` dizendo o que não foi atualizado. Corrija a causa e use "Re-run failed jobs" no run; os dois jobs podem rodar de novo (o anexo usa `--clobber`, a branch é reenviada e o PR existente é reaproveitado). Até isso, `releases/latest/download/agsync.json` não existe na release nova.

O workflow `ci` também tem o job `manifests`: passa o `actionlint` nos workflows, roda `scripts/test-manifests.sh` (manifest desatualizado, URL de outra versão, hash divergente do `SHA256SUMS`, do outro manifest ou do artefato publicado), confere os manifests de `master` contra o binário publicado e valida os schemas.

## Configuração externa

- Secret `RELEASE_PR_TOKEN`: token (fine-grained, com Contents e Pull requests de escrita neste repo) usado para abrir o PR de manifests e os PRs dos workflows `promote` e `sync-stage`. O `promote` falha sem ele, porque um merge feito com o `GITHUB_TOKEN` não dispara o workflow `release`. Sem ele, o job `manifests` e o `sync-stage` usam o `GITHUB_TOKEN`, e o GitHub não roda o `ci` em PR aberto por esse token, então os checks obrigatórios ficam pendentes; feche e reabra o PR ou faça um push nele para rodar o CI, senão o auto-merge nunca acontece. Sem o secret, abrir o PR também exige Settings > Actions > General > "Allow GitHub Actions to create and approve pull requests"; se nem isso estiver ligado, o job deixa um aviso com o link para abrir o PR na mão.
- Settings > General > "Allow auto-merge" ligado. Sem isso os PRs de manifests, `promote` e `sync-stage` são abertos, mas o merge fica manual, e os jobs `promote` e `sync-stage` falham para avisar.
- Branch padrão do repo: `stage`, para os PRs já abrirem com o alvo certo. O agendamento do `promote` roda a partir do arquivo de workflow da branch padrão.
- Secret `WINGET_TOKEN` (só usado com `WINGET_SUBMIT=true`): token classic com o escopo `public_repo` de uma conta que pode fazer fork de `microsoft/winget-pkgs`; o `wingetcreate` abre o PR a partir desse fork. Sem o secret, o job `winget` deixa um aviso e não envia nada; envie `packaging/winget` na mão com `wingetcreate submit`. O PR em `microsoft/winget-pkgs` ainda passa pela validação e revisão de lá; `winget install` só encontra a versão nova depois do merge.
- GitHub Pages com a fonte "GitHub Actions" (Settings > Pages), para o workflow `docs` publicar esta documentação. O environment `github-pages` (Settings > Environments) precisa aceitar deploy da branch `master`.
