//! A shared, dependency-free HTML report generator for the `run` face.
//!
//! Several `run` subcommands (`qa-run`, `metrics`, `event-projections`,
//! `verify-pipeline`) accept `--format json|html`. JSON is the default — it is
//! what the pipeline consumes. HTML is an *additional* artifact: a single,
//! self-contained `.html` file with embedded CSS and no external dependencies,
//! meant for a human to open in a browser.
//!
//! Fail-open contract: rendering an HTML page must never crash a `run`
//! subcommand. The caller decides what to print; if it ever cannot build a
//! page it can still emit valid JSON instead. The functions here are pure —
//! they build a `String` and never touch the filesystem or exit the process.

use std::fmt::Write as _;

/// Folha de estilo embutida: o layout padrão do Mustard (v4, mostarda e
/// carvão), o mesmo para todo documento HTML que o Mustard gera. Mora em
/// `layout.css` para ser lida e revisada como CSS, não como literal Rust.
const STYLE: &str = include_str!("layout.css");

/// Idioma do atributo `lang` quando o chamador não pede outro.
const DEFAULT_LANG: &str = "en";

/// HTML-escape a string for safe interpolation into element text / attributes.
#[must_use]
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// A self-contained HTML page: a document builder that callers feed sections
/// into. The finished string carries its own `<style>` — no external assets.
pub struct Report {
    title: String,
    subtitle: String,
    lang: String,
    body: String,
}

impl Report {
    /// Start a report page with a title and a subtitle (shown as `.meta`).
    #[must_use]
    pub fn new(title: impl Into<String>, subtitle: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: subtitle.into(),
            lang: DEFAULT_LANG.to_string(),
            body: String::new(),
        }
    }

    /// Troca o idioma do atributo `lang` do `<html>` (padrão `en`) — o resumo
    /// da spec sai em `pt-BR`, os relatórios técnicos seguem em inglês.
    // `expect`, e não `allow`: o chamador real (o resumo da spec) chega numa
    // onda seguinte; quando chegar, a expectativa deixa de se cumprir e o
    // `-D warnings` da CI obriga a remover esta linha — ela não fica esquecida.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "first caller is the spec summary page, added in a later wave")
    )]
    #[must_use]
    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = lang.into();
        self
    }

    /// Acrescenta uma seção: um `h2` (com o traço mostarda do layout) seguido
    /// do HTML interno já montado pelo chamador.
    pub fn section(&mut self, heading: &str, inner_html: &str) -> &mut Self {
        self.body.push_str("<section><h2>");
        self.body.push_str(&escape(heading));
        self.body.push_str("</h2>");
        self.body.push_str(inner_html);
        self.body.push_str("</section>");
        self
    }

    /// Append a `.card` whose body is a `<pre>` block of escaped text — used
    /// to embed the raw JSON projection alongside the rendered view.
    pub fn pre_section(&mut self, heading: &str, text: &str) -> &mut Self {
        let inner = format!("<pre>{}</pre>", escape(text));
        self.section(heading, &inner)
    }

    /// Render the finished standalone HTML document.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "<!doctype html>\n<html lang=\"{lang}\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>{title}</title><style>{style}</style></head><body><main>\
<header class=\"doc\"><p class=\"kind\">Mustard</p><h1>{title}</h1>\
<ul class=\"meta\"><li>{subtitle}</li></ul></header>{body}</main></body></html>\n",
            lang = escape(&self.lang),
            title = escape(&self.title),
            subtitle = escape(&self.subtitle),
            style = STYLE,
            body = self.body,
        )
    }
}

/// Build a `<table>` from a header row and string cells. Each row is rendered
/// verbatim as escaped text — callers that need status colouring should use
/// [`table_with_classes`] instead.
#[must_use]
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    // A moldura `.table` dá a borda arredondada e a rolagem horizontal do layout.
    let mut html = String::from("<div class=\"table\"><table><thead><tr>");
    for h in headers {
        let _ = write!(html, "<th>{}</th>", escape(h));
    }
    html.push_str("</tr></thead><tbody>");
    for row in rows {
        html.push_str("<tr>");
        for cell in row {
            let _ = write!(html, "<td>{}</td>", escape(cell));
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table></div>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_neutralizes_markup() {
        assert_eq!(escape("<b>&\"'"), "&lt;b&gt;&amp;&quot;&#39;");
    }

    #[test]
    fn report_renders_standalone_document() {
        let mut r = Report::new("QA", "spec: demo");
        r.pre_section("Raw", "{\"ok\":true}");
        let html = r.render();
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<style>"));
        // No external resource references — fully self-contained.
        assert!(!html.contains("http://") && !html.contains("https://"));
        assert!(!html.contains("src=") && !html.contains("href="));
        assert!(html.contains("spec: demo"));
        assert!(html.ends_with("</html>\n"));
    }

    /// Pares (seletor, declarações) de cada regra folha do CSS, inclusive as
    /// aninhadas em `@media`; comentários são descartados antes.
    fn css_rules(css: &str) -> Vec<(String, String)> {
        let mut clean = String::new();
        let mut rest = css;
        while let Some(start) = rest.find("/*") {
            clean.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            rest = after.find("*/").map_or("", |end| &after[end + 2..]);
        }
        clean.push_str(rest);

        let mut rules = Vec::new();
        let mut prelude_start = 0;
        let mut open: Option<(String, usize)> = None;
        for (i, c) in clean.char_indices() {
            match c {
                '{' => {
                    open = Some((clean[prelude_start..i].trim().to_string(), i + 1));
                    prelude_start = i + 1;
                }
                '}' => {
                    if let Some((selector, body_start)) = open.take() {
                        rules.push((selector, clean[body_start..i].replace([' ', '\n'], "")));
                    }
                    prelude_start = i + 1;
                }
                _ => {}
            }
        }
        rules
    }

    /// Verdadeiro quando algum composto do seletor mira um `li`
    /// (`li`, `li.done`, `li::before`...).
    fn targets_li(selector: &str) -> bool {
        selector
            .split(|c: char| c.is_whitespace() || matches!(c, ',' | '>' | '+' | '~'))
            .any(|compound| {
                compound
                    .strip_prefix("li")
                    .is_some_and(|tail| tail.is_empty() || tail.starts_with(['.', ':', '[', '#']))
            })
    }

    #[test]
    fn report_renders_the_standard_mustard_layout() {
        let mut r = Report::new("Resumo da spec", "spec: demo").with_lang("pt-BR");
        r.section("Tabela", &table(&["A"], &[vec!["`x`".into()]]));
        let html = r.render();

        // Idioma pedido no <html>; sem pedido, vale o padrão `en`.
        assert!(html.contains("<html lang=\"pt-BR\">"), "lang pedido ausente");
        assert!(Report::new("QA", "x").render().contains("<html lang=\"en\">"));

        let css = html
            .split_once("<style>")
            .and_then(|(_, tail)| tail.split_once("</style>"))
            .map(|(style, _)| style)
            .expect("página sem <style>");

        // Tokens mostarda e carvão, claro e escuro.
        for token in [
            "#FAFAF7", "#2B2B29", "#E1AD01", "#8A6700", "#FBF1CF", "#2E2E2B", "#1C1C1A", "#ECEAE3",
            "#E8B923", "#121211",
        ] {
            assert!(css.contains(token), "token {token} ausente do layout");
        }
        assert!(css.contains("\"Geist\"") && css.contains("\"Geist Mono\""));
        assert!(css.contains("max-width:860px"));

        // Estrutura do layout: faixa carvão no topo, tabela em moldura.
        assert!(html.contains("<header class=\"doc\">"));
        assert!(html.contains("<div class=\"table\"><table>"));

        let rules = css_rules(css);
        // Código inline nunca quebra no meio da palavra.
        let code = rules.iter().find(|(sel, _)| sel == "code").expect("regra code ausente");
        assert!(code.1.contains("white-space:nowrap"), "code inline sem nowrap: {}", code.1);
        assert!(!css.contains("overflow-wrap:anywhere"), "overflow-wrap:anywhere proibido");

        // Nenhum li (nem pseudo-elemento dele) vira grid.
        for (selector, decls) in &rules {
            assert!(
                !(targets_li(selector) && decls.contains("display:grid")),
                "li em grid: {selector}{{{decls}}}"
            );
        }
    }

    #[test]
    fn table_builds_rows() {
        let html = table(&["A", "B"], &[vec!["1".into(), "2".into()]]);
        assert!(html.contains("<th>A</th>"));
        assert!(html.contains("<td>1</td>"));
    }
}
