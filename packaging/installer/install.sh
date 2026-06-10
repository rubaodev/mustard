#!/usr/bin/env sh
# ============================================================================
# Mustard — instalador para teste (Linux)
#
# Instala os binários pré-compilados do Mustard (scan, mustard-rt, mustard-mcp,
# mustard), o rtk empacotado e a carga templates/. NÃO precisa do toolchain Rust
# — são binários já compilados.
#
# Layout após instalar (auto-contido, fácil de remover com `rm -rf ~/.mustard`):
#   ~/.mustard/bin/        -> entra no PATH (mustard, mustard-rt, …, rtk)
#   ~/.mustard/templates/  -> resolvido pelo mustard como <pasta-do-exe>/../templates
#
# Uso:
#   ./install.sh                  # instala binários + ajusta PATH (sem init)
#   ./install.sh /caminho/projeto # também roda `mustard init` nesse projeto
#   MUSTARD_PREFIX=/opt/mustard ./install.sh   # local de instalação custom
# ============================================================================
set -eu

# --- localiza o pacote (a pasta deste script) -------------------------------
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PKG_BIN="$SCRIPT_DIR/bin"
PKG_TEMPLATES="$SCRIPT_DIR/templates"

[ -d "$PKG_BIN" ] || { echo "erro: $PKG_BIN não encontrado — rode o install.sh de dentro do pacote descompactado." >&2; exit 1; }
[ -d "$PKG_TEMPLATES" ] || { echo "erro: $PKG_TEMPLATES não encontrado — pacote incompleto." >&2; exit 1; }

# --- local de instalação ----------------------------------------------------
PREFIX="${MUSTARD_PREFIX:-$HOME/.mustard}"
BIN_DIR="$PREFIX/bin"
TEMPLATES_DIR="$PREFIX/templates"

echo "==> Instalando o Mustard em $PREFIX"
mkdir -p "$BIN_DIR"

# --- copia os binários ------------------------------------------------------
for f in "$PKG_BIN"/*; do
  [ -e "$f" ] || continue
  cp -f "$f" "$BIN_DIR/"
  chmod +x "$BIN_DIR/$(basename "$f")"
done

# --- copia a carga de templates ---------------------------------------------
rm -rf "$TEMPLATES_DIR"
cp -R "$PKG_TEMPLATES" "$TEMPLATES_DIR"

# Faz esta sessão enxergar os binários novos (o init abaixo precisa de rtk +
# mustard). Inclui os locais usuais onde o instalador oficial do rtk cai.
PATH="$BIN_DIR:$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export PATH

# --- garante o rtk ----------------------------------------------------------
# rtk é dependência obrigatória do `mustard init` e dos hooks de Bash em runtime.
if command -v rtk >/dev/null 2>&1; then
  echo "  rtk presente: $(command -v rtk)"
else
  echo "  rtk não encontrado — tentando o instalador oficial (baixa binário pré-compilado)…"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/master/install.sh | sh || true
  fi
  if ! command -v rtk >/dev/null 2>&1; then
    echo "  ! rtk ainda não está no PATH. Instale manualmente e rode de novo:" >&2
    echo "      curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/master/install.sh | sh" >&2
  fi
fi

# --- persiste o PATH --------------------------------------------------------
add_path_line="export PATH=\"$BIN_DIR:\$PATH\""
marker='# >>> mustard bin >>>'
for rc in "$HOME/.profile" "$HOME/.bashrc"; do
  [ -e "$rc" ] || touch "$rc"
  if ! grep -qF "$marker" "$rc" 2>/dev/null; then
    printf '\n%s\n%s\n' "$marker" "$add_path_line" >> "$rc"
    echo "  entrada de PATH adicionada em $rc"
  fi
done

# --- opcional: prepara um projeto ------------------------------------------
TARGET="${1:-}"
if [ -n "$TARGET" ]; then
  [ -d "$TARGET" ] || { echo "erro: projeto-alvo não existe: $TARGET" >&2; exit 1; }
  TARGET=$(CDPATH= cd -- "$TARGET" && pwd)
  echo "==> Rodando 'mustard init' em $TARGET"
  ( cd "$TARGET" && MUSTARD_TEMPLATES_DIR="$TEMPLATES_DIR" "$BIN_DIR/mustard" init --yes )
else
  echo
  echo "==> Binários instalados. Para preparar um projeto:"
  echo "      cd /caminho/do/seu/projeto && mustard init"
fi

echo
echo "==> Pronto. Abra um NOVO terminal (ou: source ~/.profile) para o 'mustard' entrar no PATH."
