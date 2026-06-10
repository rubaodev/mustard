# Mustard no Linux — tutorial de instalação

Este tutorial explica, passo a passo, como instalar o pacote de teste do
Mustard num Linux. O pacote traz **binários já compilados** — você não precisa
instalar Rust, Node ou qualquer ferramenta de desenvolvimento.

O que será instalado (tudo dentro de uma única pasta, fácil de remover):

```
~/.mustard/bin/        mustard, mustard-rt, mustard-mcp, scan e rtk  (entra no PATH)
~/.mustard/templates/  a carga que o `mustard init` copia para os projetos
```

---

## 1. Pré-requisitos

| Requisito | Como verificar |
|---|---|
| Linux x64 com glibc 2.31+ (Ubuntu 20.04+, Debian 11+, Fedora 33+) | `ldd --version` — a primeira linha mostra a versão |
| Claude Code instalado e logado (o Mustard trabalha dentro dele) | `claude --version` |
| `curl` (só usado se o rtk precisar ser baixado) | `curl --version` |
| `git` (opcional, recomendado) | `git --version` |

Se ainda não tiver o Claude Code, instale com:

```sh
curl -fsSL https://claude.ai/install.sh | bash
```

e faça login uma vez com `claude` (guia completo em <https://docs.claude.com/claude-code>).

---

## 2. Baixar e descompactar o pacote

Copie o arquivo `mustard-linux-x64.tar.gz` para qualquer pasta (por exemplo,
`~/Downloads`) e descompacte:

```sh
cd ~/Downloads
tar -xzf mustard-linux-x64.tar.gz
cd mustard-linux-x64
```

Dentro da pasta você verá `bin/`, `templates/`, `install.sh` e `README.txt`.

---

## 3. Instalar

Há dois modos — escolha um:

**a) Só instalar os binários** (e ajustar o PATH):

```sh
./install.sh
```

**b) Instalar e já preparar um projeto seu para testar** (roda o
`mustard init` no projeto indicado):

```sh
./install.sh /caminho/do/seu/projeto
```

O instalador:

1. copia os binários para `~/.mustard/bin` e os templates para `~/.mustard/templates`;
2. adiciona `~/.mustard/bin` ao PATH em `~/.profile` e `~/.bashrc` (bloco marcado com `# >>> mustard bin >>>`);
3. garante o `rtk` — o pacote já o traz; se faltar, baixa o instalador oficial;
4. se você passou um projeto, roda `mustard init` nele (cria a pasta `.claude/` e o `mustard.json`).

> Quer instalar em outro lugar? Use `MUSTARD_PREFIX=/opt/mustard ./install.sh`.

---

## 4. Abrir um novo terminal e verificar

O PATH só vale para terminais novos. Abra um **novo terminal** (ou rode
`source ~/.profile`) e confira:

```sh
mustard --version
mustard-rt --version
rtk --version
```

Os três devem responder com a versão. Se algum disser
`command not found`, veja a seção de problemas abaixo.

---

## 5. Preparar um projeto (se ainda não preparou)

Em qualquer projeto que você queira testar:

```sh
cd /caminho/do/seu/projeto
mustard init
```

Isso cria a pasta `.claude/` (hooks, skills e configuração) e o
`mustard.json` na raiz. A partir daí é só **abrir o Claude Code normalmente
dentro do projeto** — os hooks do Mustard já estão ligados via
`.claude/settings.json`; nenhum passo extra é necessário.

Comandos úteis dentro do Claude Code: `/scan` (mapeia o projeto),
`/feature` (pipeline de feature), `/bugfix`, `/status`.

---

## 6. Problemas comuns

**`mustard: command not found`**
O terminal ainda não recarregou o PATH. Abra um novo terminal ou rode
`source ~/.profile`. Se persistir, confira se o bloco
`# >>> mustard bin >>>` existe no `~/.profile` ou `~/.bashrc` — seu shell pode
usar outro arquivo (zsh usa `~/.zshrc`; copie a linha `export PATH=...` para lá).

**Erro tipo `GLIBC_2.31' not found`**
Sua distribuição é mais antiga que o mínimo suportado (glibc 2.31).
Atualize a distro ou rode numa máquina/container mais novo (Ubuntu 20.04+).

**`rtk` não foi instalado**
Instale manualmente e rode o `install.sh` de novo:

```sh
curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/master/install.sh | sh
```

**`Permission denied` ao rodar `./install.sh`**
O bit de execução se perdeu no download. Rode `chmod +x install.sh` e tente de novo.

---

## 7. Desinstalar

```sh
rm -rf ~/.mustard
```

e remova o bloco `# >>> mustard bin >>>` (duas linhas) do `~/.profile` e do
`~/.bashrc`. Em projetos testados, a pasta `.claude/` e o `mustard.json`
podem ser apagados à vontade.
