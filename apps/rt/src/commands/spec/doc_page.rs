//! `mustard-rt run doc-page --title <t> --body <arquivo> [--subtitle <s>]
//! [--kind <rótulo>] [--lang <bcp47>] --out <caminho>` — embrulha um corpo HTML
//! no layout padrão do Mustard e grava a página.
//!
//! ## Por que existe
//!
//! O layout do Mustard (v4, mostarda e carvão) só existia dentro do gerador da
//! página da spec (E-5). Em 10/09/2026, noutro projeto, o assistente publicou
//! uma página com visual próprio: a regra "todo documento usa o layout" vivia
//! na memória de uma máquina, e memória não viaja. Este comando é a porta que a
//! regra injetada do material manda usar (K-6): o assistente escreve só o
//! corpo, e cabeçalho, fontes e cores vêm de [`crate::report::Report`].
//!
//! ## Contrato
//!
//! Saída: `{ok, path}`, com `path` exatamente como `--out` foi passado (barras
//! normais) — nada de caminho absoluto da máquina. Exit 0 quando grava; 1 numa
//! recusa (título vazio, corpo ilegível, disco sem escrita), que sai com
//! `error` e `remedy` e não grava nada.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::report::Report;

/// Options for `mustard-rt run doc-page`.
pub struct DocPageOpts {
    /// O título da página, no `<title>` e no `<h1>`.
    pub title: String,
    /// O arquivo com o fragmento HTML que vai dentro de `<main>`.
    pub body: PathBuf,
    /// Uma linha solta sob o título, na faixa `.meta`.
    pub subtitle: Option<String>,
    /// O que vem depois de `Mustard · ` na faixa do cabeçalho.
    pub kind: Option<String>,
    /// Idioma BCP-47 do atributo `lang`; sem ele vale o padrão do layout.
    pub lang: Option<String>,
    /// Onde gravar a página; diretórios ausentes são criados.
    pub out: PathBuf,
}

/// O relatório JSON.
#[derive(Debug, Serialize)]
pub(crate) struct DocPageReport {
    pub(crate) ok: bool,
    pub(crate) path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) remedy: Option<String>,
}

/// Uma recusa: o código do erro e o que fazer.
type Refusal = (&'static str, &'static str);

const EMPTY_TITLE: Refusal = (
    "empty_title",
    "pass the page title with --title — it heads the page and names the tab",
);
const UNREADABLE_BODY: Refusal = (
    "unreadable_body",
    "pass --body a readable UTF-8 file holding the HTML fragment that goes inside <main>",
);
const WRITE_FAILED: Refusal = (
    "write_failed",
    "the page could not be written — check that --out points at a writable place",
);

impl DocPageReport {
    fn refused((error, remedy): Refusal) -> Self {
        Self {
            ok: false,
            path: String::new(),
            error: Some(error.to_string()),
            remedy: Some(remedy.to_string()),
        }
    }
}

/// Monta a página e a grava em `opts.out`. Toda recusa acontece antes da
/// gravação, então uma recusa nunca deixa página pela metade.
#[must_use]
pub(crate) fn write_page(opts: &DocPageOpts) -> DocPageReport {
    let title = opts.title.trim();
    if title.is_empty() {
        return DocPageReport::refused(EMPTY_TITLE);
    }
    let Ok(body) = std::fs::read_to_string(&opts.body) else {
        return DocPageReport::refused(UNREADABLE_BODY);
    };
    let html = render(title, &body, opts);
    if mustard_core::io::fs::write_atomic(&opts.out, html.as_bytes()).is_err() {
        return DocPageReport::refused(WRITE_FAILED);
    }
    DocPageReport {
        ok: true,
        path: display_path(&opts.out),
        error: None,
        remedy: None,
    }
}

/// O corpo vai cru para dentro de `<main>`, depois do cabeçalho do layout: é
/// HTML que o assistente já montou, e escapá-lo mostraria as marcas.
fn render(title: &str, body: &str, opts: &DocPageOpts) -> String {
    let mut report = Report::new(title, opts.subtitle.as_deref().unwrap_or(""));
    if let Some(lang) = non_blank(opts.lang.as_deref()) {
        report = report.with_lang(lang);
    }
    if let Some(kind) = non_blank(opts.kind.as_deref()) {
        report = report.with_kind(kind);
    }
    report.raw(body);
    report.render()
}

fn non_blank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

/// O caminho como foi pedido, com barras normais: o relatório lê igual em toda
/// plataforma.
fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// CLI entry — `mustard-rt run doc-page`.
pub fn run(opts: &DocPageOpts) {
    let report = write_page(opts);
    let body = serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string());
    println!("{body}");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    std::process::exit(i32::from(!report.ok));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn opts(root: &Path, title: &str) -> DocPageOpts {
        DocPageOpts {
            title: title.to_string(),
            body: root.join("corpo.html"),
            subtitle: Some("onda 4".to_string()),
            kind: Some("plano".to_string()),
            lang: Some("pt-BR".to_string()),
            out: root.join("paginas/plano.html"),
        }
    }

    /// AC-8 — o corpo sai no layout do Mustard: o CSS do layout, o título no
    /// cabeçalho e o corpo, intacto, dentro de `<main>` logo depois dele.
    #[test]
    fn doc_page_wraps_the_body_in_the_mustard_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let body = "<section><h2>Passos</h2><p>O corpo escrito pelo assistente.</p></section>";
        fs::write(root.join("corpo.html"), body).unwrap();

        let report = write_page(&opts(root, "Plano da onda"));
        assert!(report.ok, "{report:?}");
        assert!(report.path.ends_with("paginas/plano.html"), "{}", report.path);
        let html = fs::read_to_string(root.join("paginas/plano.html")).unwrap();

        let css = html
            .split_once("<style>")
            .and_then(|(_, tail)| tail.split_once("</style>"))
            .map(|(style, _)| style)
            .expect("página sem <style>");
        for token in ["#E1AD01", "#2B2B29", "max-width:860px", "font-family:\"Geist\""] {
            assert!(css.contains(token), "o CSS do layout não traz {token}");
        }

        assert!(html.contains("<html lang=\"pt-BR\">"), "{html}");
        assert!(html.contains("<title>Plano da onda</title>"));
        assert!(
            html.contains(
                "<header class=\"doc\"><p class=\"kind\">Mustard · plano</p><h1>Plano da onda</h1>"
            ),
            "{html}"
        );
        assert!(html.contains("<li>onda 4</li>"));

        let main = html
            .split_once("<main>")
            .and_then(|(_, tail)| tail.split_once("</main>"))
            .map(|(inner, _)| inner)
            .expect("página sem <main>");
        let after_header = main.split_once("</header>").map(|(_, tail)| tail).expect("sem cabeçalho");
        assert_eq!(after_header, body, "o corpo vai cru, inteiro, depois do cabeçalho");
    }

    /// Título vazio ou corpo ilegível: recusa com o código do erro e nada gravado.
    #[test]
    fn doc_page_refuses_a_blank_title_or_an_unreadable_body_without_writing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("corpo.html"), "<p>x</p>").unwrap();

        let blank = write_page(&opts(root, "   "));
        assert!(!blank.ok);
        assert_eq!(blank.error.as_deref(), Some("empty_title"));

        let mut missing = opts(root, "Plano");
        missing.body = root.join("nao-existe.html");
        let unreadable = write_page(&missing);
        assert!(!unreadable.ok);
        assert_eq!(unreadable.error.as_deref(), Some("unreadable_body"));

        assert!(!root.join("paginas").exists(), "uma recusa não grava nada");
    }
}
