Mustard — pacote de teste (sem dashboard)
=========================================

Este pacote contém os binários já compilados do Mustard. Você NÃO precisa
instalar o Rust nem compilar nada — é só rodar o instalador.

Conteúdo:
  bin/         binários: mustard, mustard-rt, mustard-mcp, scan e rtk
  templates/   a carga que o `mustard init` copia para .claude/
  install.sh   instalador (Linux)
  install.ps1  instalador (Windows)


Requisitos
----------
- Linux:   glibc 2.31 ou superior (Ubuntu 20.04+, Debian 11+, Fedora 33+).
           `git` é opcional (recomendado).
- Windows: Windows 10/11.
- Em ambos: nenhum toolchain de desenvolvimento é necessário.


Como instalar
-------------

LINUX
  1. Descompacte:   tar -xzf mustard-linux-x64.tar.gz && cd mustard-linux-x64
  2a. Só instalar os binários (e ajustar o PATH):
        ./install.sh
  2b. Instalar E já preparar um projeto seu para testar:
        ./install.sh /caminho/do/seu/projeto
  3. Abra um NOVO terminal (ou: source ~/.profile).

WINDOWS (PowerShell)
  1. Descompacte o .zip e entre na pasta.
  2a. Só instalar os binários (e ajustar o PATH):
        .\install.ps1
  2b. Instalar E já preparar um projeto seu para testar:
        .\install.ps1 -Target C:\caminho\do\projeto
  3. Abra um NOVO terminal.


O que o instalador faz
----------------------
- Copia os binários para  ~/.mustard/bin  (Windows: %USERPROFILE%\.mustard\bin)
  e os templates para     ~/.mustard/templates.
- Adiciona essa pasta bin ao seu PATH (de forma persistente).
- Garante o rtk (o pacote já traz o rtk; no Linux, se faltar, baixa o oficial).
- Se você passar um projeto, roda `mustard init` nele (cria a pasta .claude/).


Como usar depois
----------------
- Num projeto preparado:  rode o Claude Code normalmente — os hooks do Mustard
  já estão ligados via .claude/settings.json.
- Para preparar outro projeto:  cd <projeto> && mustard init
- Versão instalada:  mustard --version   /   mustard-rt --version


Como remover
------------
- Linux:   rm -rf ~/.mustard   e tire a linha "# >>> mustard bin >>>" do
           ~/.profile e ~/.bashrc.
- Windows: apague %USERPROFILE%\.mustard e remova a entrada do PATH de usuário.
- Em um projeto testado, a pasta .claude/ pode ser apagada à vontade.
