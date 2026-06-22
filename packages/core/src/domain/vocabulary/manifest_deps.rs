//! `manifest_deps` — declared-dependency extraction + dependency→category
//! classification, the manifest-driven replacement for the source-content
//! framework scan.
//!
//! ## Why this module exists
//!
//! [`super::frameworks`] answers *"does this source TEXT mention a framework
//! token?"* by scanning code content. That question is the wrong one for the
//! registry's `_patterns.{stack}.frameworks` label: a file that merely *quotes*
//! or *describes* a framework token (the case of framework-detection software
//! whose own source lists `pgTable(`/`@Entity` as patterns) manufactures a
//! false positive. The only authoritative statement that a subproject *uses* a
//! framework is the project's **build manifest** — the declared dependency set.
//!
//! This module reads that declared set, language-aware (keyed on which manifest
//! files are present — never on a language *name* in a logic branch), and
//! classifies each dependency name into a [`super::frameworks::FrameworkCategory`]
//! via an EXTERNAL vocabulary loaded from `.claude/vocab/frameworks.toml`. The
//! classifier knows nothing baked into the binary: with no vocab file present
//! classification is simply empty (correct, not an error). A dependency the
//! vocab does not map is recorded as an *unclassified* gap for the (future)
//! web-fetch rung to resolve — never fabricated into a category.
//!
//! ## On-disk schema (`.claude/vocab/frameworks.toml`)
//!
//! A NEW table-array distinct from the `[[signal]]` code-pattern schema
//! [`super::frameworks`] consumes (the two coexist in the same file — serde
//! ignores the table each parser does not own):
//!
//! ```toml
//! [[dependency]]
//! category = "orm"
//! # exact dependency names (case-insensitive)
//! names = ["diesel", "sea-orm", "prisma", "drizzle-orm"]
//! # name prefixes / namespaces — a dependency whose name starts with one of
//! # these (e.g. a scoped `@nestjs/common`) classifies too.
//! prefixes = []
//!
//! [[dependency]]
//! category = "framework"
//! names = ["axum", "actix-web", "express", "fastify", "django", "gin"]
//!
//! [[dependency]]
//! category = "di"
//! names = []
//! prefixes = ["@nestjs", "@angular"]
//! ```
//!
//! Matching is by declared NAME, not by source substring — so a pure library
//! that only *mentions* `axum` in a doc comment carries no label, while a
//! project that declares `axum` in `[dependencies]` does.

use super::frameworks::FrameworkCategory;
use super::VocabError;
use crate::io::fs;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

/// The default on-disk vocabulary name resolved under `.claude/vocab/`.
/// [`DependencyClassifier::load`] looks for `.claude/vocab/frameworks.toml`.
pub const DEFAULT_FRAMEWORKS_NAME: &str = "frameworks";

// ---------------------------------------------------------------------------
// Declared-dependency extraction
// ---------------------------------------------------------------------------

/// Extract the set of DECLARED dependency names for the subproject rooted at
/// `sub_root`, language-aware via which manifest files are present.
///
/// Supported manifests (a subproject may carry more than one; every declared
/// name across all of them is unioned):
/// - **Rust** `Cargo.toml`: `[dependencies]` / `[dev-dependencies]` /
///   `[build-dependencies]` keys.
/// - **JS/TS** `package.json`: `dependencies` + `devDependencies` keys.
/// - **C#** `*.csproj`: `<PackageReference Include="...">` values.
/// - **Go** `go.mod`: `require` module paths.
/// - **Python** `pyproject.toml`: `[project].dependencies` +
///   `[tool.poetry.dependencies]` keys.
///
/// Names are returned verbatim (case preserved); the classifier lower-cases for
/// comparison. Absent / unreadable manifests contribute nothing — a project
/// with no manifest yields an empty set, which is correct (no declared stack).
#[must_use]
pub fn declared_dependencies(sub_root: &Path) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    cargo_toml_deps(sub_root, &mut out);
    package_json_deps(sub_root, &mut out);
    csproj_deps(sub_root, &mut out);
    go_mod_deps(sub_root, &mut out);
    pyproject_deps(sub_root, &mut out);
    out
}

/// Rust `Cargo.toml` — collect the keys of every dependency table.
fn cargo_toml_deps(root: &Path, out: &mut BTreeSet<String>) {
    let Ok(text) = fs::read_to_string(root.join("Cargo.toml")) else {
        return;
    };
    let Ok(doc) = toml::from_str::<toml::Value>(&text) else {
        return;
    };
    for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(deps) = doc.get(table).and_then(toml::Value::as_table) {
            for name in deps.keys() {
                out.insert(name.clone());
            }
        }
    }
}

/// JS/TS `package.json` — dependency + devDependency keys.
fn package_json_deps(root: &Path, out: &mut BTreeSet<String>) {
    let Ok(text) = fs::read_to_string(root.join("package.json")) else {
        return;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    for section in ["dependencies", "devDependencies"] {
        if let Some(obj) = json.get(section).and_then(serde_json::Value::as_object) {
            for name in obj.keys() {
                out.insert(name.clone());
            }
        }
    }
}

/// C# `*.csproj` — `<PackageReference Include="Name" ...>` values, read from
/// every `.csproj` directly inside `root`. Substring extraction (no XML
/// dependency): find each `Include="` and read up to the closing quote.
fn csproj_deps(root: &Path, out: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries {
        if entry.is_dir || !entry.file_name.ends_with(".csproj") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&entry.path) else {
            continue;
        };
        // Only `<PackageReference Include="...">` declares a NuGet dependency.
        for fragment in text.split("<PackageReference").skip(1) {
            if let Some(name) = attribute_value(fragment, "Include=\"") {
                out.insert(name);
            }
        }
    }
}

/// Read the value of `attr` (e.g. `Include="`) starting at the front of
/// `fragment`, up to the next `"`. The fragment must begin inside one element,
/// so the first occurrence of `attr` is that element's attribute.
fn attribute_value(fragment: &str, attr: &str) -> Option<String> {
    let start = fragment.find(attr)? + attr.len();
    let rest = &fragment[start..];
    let end = rest.find('"')?;
    let value = rest[..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// Go `go.mod` — module paths inside `require` (both the block form and the
/// single-line `require path version` form).
fn go_mod_deps(root: &Path, out: &mut BTreeSet<String>) {
    let Ok(text) = fs::read_to_string(root.join("go.mod")) else {
        return;
    };
    let mut in_block = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if in_block {
            if line.starts_with(')') {
                in_block = false;
                continue;
            }
            if let Some(path) = line.split_whitespace().next() {
                out.insert(path.to_string());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("require") {
            let rest = rest.trim();
            if rest.starts_with('(') {
                in_block = true;
                continue;
            }
            // Single-line form: `require module/path v1.2.3`.
            if let Some(path) = rest.split_whitespace().next() {
                if !path.is_empty() {
                    out.insert(path.to_string());
                }
            }
        }
    }
}

/// Python `pyproject.toml` — PEP 621 `[project].dependencies` (a list of
/// requirement strings) plus `[tool.poetry.dependencies]` (a table keyed by
/// package name). The requirement string is reduced to its leading package
/// name (`fastapi>=0.110` → `fastapi`).
fn pyproject_deps(root: &Path, out: &mut BTreeSet<String>) {
    let Ok(text) = fs::read_to_string(root.join("pyproject.toml")) else {
        return;
    };
    let Ok(doc) = toml::from_str::<toml::Value>(&text) else {
        return;
    };
    // PEP 621: `[project] dependencies = ["fastapi>=0.110", ...]`.
    if let Some(list) = doc
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(toml::Value::as_array)
    {
        for item in list {
            if let Some(req) = item.as_str() {
                if let Some(name) = requirement_package_name(req) {
                    out.insert(name);
                }
            }
        }
    }
    // Poetry: `[tool.poetry.dependencies]` table keyed by package name.
    if let Some(table) = doc
        .get("tool")
        .and_then(|t| t.get("poetry"))
        .and_then(|p| p.get("dependencies"))
        .and_then(toml::Value::as_table)
    {
        for name in table.keys() {
            out.insert(name.clone());
        }
    }
}

/// Reduce a PEP 508 requirement string to its leading package name. Stops at
/// the first version-spec / extras / marker delimiter (` `, `=`, `<`, `>`, `~`,
/// `!`, `[`, `;`, `(`). Returns `None` for an empty result.
fn requirement_package_name(req: &str) -> Option<String> {
    let name: String = req
        .trim()
        .chars()
        .take_while(|c| !matches!(c, ' ' | '=' | '<' | '>' | '~' | '!' | '[' | ';' | '('))
        .collect();
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

// ---------------------------------------------------------------------------
// Dependency→category vocabulary
// ---------------------------------------------------------------------------

/// One `[[dependency]]` table-array entry: a category plus the exact dependency
/// names and name prefixes that classify into it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DependencyRule {
    /// Which category these dependencies belong to. Closed enum — an unknown
    /// `category` value surfaces as [`VocabError::InvalidToml`].
    pub category: FrameworkCategory,
    /// Exact dependency names (matched case-insensitively).
    #[serde(default)]
    pub names: Vec<String>,
    /// Name prefixes / namespaces — a dependency whose lower-cased name starts
    /// with one of these classifies into `category`. Empty by default.
    #[serde(default)]
    pub prefixes: Vec<String>,
}

/// Top-level document for the dependency→category vocabulary. The
/// `[[dependency]]` table array is the only key this parser reads; any
/// `[[signal]]` blocks (the code-pattern schema) are silently ignored, so both
/// schemas coexist in `frameworks.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct DependencyVocabularyDoc {
    /// Every `[[dependency]]` table entry, in document order.
    #[serde(default, rename = "dependency")]
    pub rules: Vec<DependencyRule>,
}

impl DependencyVocabularyDoc {
    /// Parse a dependency vocabulary TOML document. Pure on `&str`.
    ///
    /// # Errors
    /// Returns [`VocabError::InvalidToml`] when the input cannot be
    /// deserialised (bad `category`, malformed table array, …).
    pub fn parse_str(raw: &str) -> Result<Self, VocabError> {
        toml::from_str::<Self>(raw).map_err(|e| VocabError::InvalidToml(e.to_string()))
    }
}

/// A built dependency classifier: maps a declared dependency name to its
/// [`FrameworkCategory`] via exact-name and prefix rules loaded from the
/// project's `.claude/vocab/frameworks.toml`.
///
/// An ABSENT vocab file yields an EMPTY classifier — classification then
/// returns no categories, which is correct (the binary bakes no framework
/// knowledge). Construct via [`DependencyClassifier::load`] or
/// [`DependencyClassifier::from_doc`].
#[derive(Debug, Clone, Default)]
pub struct DependencyClassifier {
    rules: Vec<DependencyRule>,
}

/// The outcome of classifying a subproject's declared dependencies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyClassification {
    /// The distinct [`FrameworkCategory`] tags (`orm` / `framework` / `di`)
    /// present among the declared dependencies, sorted + deduplicated.
    pub categories: BTreeSet<String>,
    /// Declared dependency names the vocabulary did not classify — a GAP
    /// surfaced for the (future) web-fetch rung, never fabricated into a
    /// category. Sorted + deduplicated.
    pub unclassified: BTreeSet<String>,
}

impl DependencyClassifier {
    /// Build a classifier from a parsed document.
    #[must_use]
    pub fn from_doc(doc: DependencyVocabularyDoc) -> Self {
        Self { rules: doc.rules }
    }

    /// Load the dependency classifier for `project_root`, reading
    /// `.claude/vocab/{DEFAULT_FRAMEWORKS_NAME}.toml`.
    ///
    /// Resolution:
    /// - File ABSENT ⇒ an EMPTY classifier (no labels). The binary bakes no
    ///   framework knowledge, so "no vocab" is correctly "no classification",
    ///   not an error.
    /// - File present ⇒ its `[[dependency]]` rules drive classification.
    ///
    /// # Errors
    /// [`VocabError::InvalidToml`] when the file exists but cannot be parsed
    /// (a malformed override is a real configuration bug), or
    /// [`VocabError::Io`] on a non-`NotFound` read failure.
    pub fn load(project_root: &Path) -> Result<Self, VocabError> {
        let path = project_root
            .join(".claude")
            .join("vocab")
            .join(format!("{DEFAULT_FRAMEWORKS_NAME}.toml"));
        match std::fs::read_to_string(&path) {
            Ok(raw) => Ok(Self::from_doc(DependencyVocabularyDoc::parse_str(&raw)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(VocabError::Io(e.to_string())),
        }
    }

    /// `true` when no rule is loaded (the vocab file was absent).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Classify a single dependency name, returning its category when an exact
    /// name (case-insensitive) or a name prefix rule matches. Document order is
    /// priority order: the first matching rule wins.
    #[must_use]
    pub fn classify_one(&self, dependency: &str) -> Option<FrameworkCategory> {
        let lc = dependency.trim().to_ascii_lowercase();
        if lc.is_empty() {
            return None;
        }
        for rule in &self.rules {
            if rule.names.iter().any(|n| n.trim().eq_ignore_ascii_case(&lc)) {
                return Some(rule.category);
            }
            if rule
                .prefixes
                .iter()
                .any(|p| !p.is_empty() && lc.starts_with(&p.to_ascii_lowercase()))
            {
                return Some(rule.category);
            }
        }
        None
    }

    /// Classify a whole declared-dependency set into the present categories +
    /// the unclassified gap. An empty classifier yields no categories and
    /// reports every dependency as unclassified.
    #[must_use]
    pub fn classify(&self, dependencies: &BTreeSet<String>) -> DependencyClassification {
        let mut out = DependencyClassification::default();
        for dep in dependencies {
            match self.classify_one(dep) {
                Some(cat) => {
                    out.categories.insert(cat.as_str().to_string());
                }
                None => {
                    out.unclassified.insert(dep.clone());
                }
            }
        }
        out
    }
}

/// Classify the DECLARED dependencies of the subproject at `sub_root` into the
/// framework / ORM / DI categories present, honouring the project-local
/// `.claude/vocab/frameworks.toml` dependency vocabulary.
///
/// This is the manifest-driven replacement for source-content framework
/// scanning: the label is computed from what the manifest DECLARES, classified
/// through an EXTERNAL vocab. With no vocab file present the categories are
/// empty (correct) and every declared dependency is reported as unclassified.
///
/// Fail-open on a malformed vocab: a parse error degrades to an empty
/// classifier (no categories) rather than aborting the scan, matching the
/// pipeline's stance; the every-dependency-unclassified result then surfaces the
/// gap.
#[must_use]
pub fn classify_subproject_dependencies(sub_root: &Path) -> DependencyClassification {
    let deps = declared_dependencies(sub_root);
    let classifier = DependencyClassifier::load(sub_root).unwrap_or_default();
    classifier.classify(&deps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(root: &Path, rel: &str, body: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    // -----------------------------------------------------------------------
    // Declared-dependency extraction
    // -----------------------------------------------------------------------

    #[test]
    fn cargo_toml_collects_all_dependency_tables() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = \"x\"\n\
             [dependencies]\naxum = \"0.7\"\nserde = { version = \"1\" }\n\
             [dev-dependencies]\ntempfile = \"3\"\n\
             [build-dependencies]\ncc = \"1\"\n",
        );
        let deps = declared_dependencies(dir.path());
        assert!(deps.contains("axum"));
        assert!(deps.contains("serde"));
        assert!(deps.contains("tempfile"));
        assert!(deps.contains("cc"));
    }

    #[test]
    fn package_json_collects_deps_and_dev_deps() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{ "dependencies": { "express": "^4", "@nestjs/common": "10" },
                "devDependencies": { "vitest": "1" } }"#,
        );
        let deps = declared_dependencies(dir.path());
        assert!(deps.contains("express"));
        assert!(deps.contains("@nestjs/common"));
        assert!(deps.contains("vitest"));
    }

    #[test]
    fn csproj_collects_package_references() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "App.csproj",
            "<Project Sdk=\"Microsoft.NET.Sdk.Web\">\n  <ItemGroup>\n\
             <PackageReference Include=\"Microsoft.EntityFrameworkCore\" Version=\"8.0\" />\n\
             <PackageReference Include=\"Serilog\" Version=\"3.0\" />\n\
             </ItemGroup>\n</Project>\n",
        );
        let deps = declared_dependencies(dir.path());
        assert!(deps.contains("Microsoft.EntityFrameworkCore"));
        assert!(deps.contains("Serilog"));
    }

    #[test]
    fn go_mod_collects_block_and_single_require() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "go.mod",
            "module example.com/app\n\ngo 1.22\n\n\
             require github.com/gin-gonic/gin v1.9.1\n\n\
             require (\n\tgithub.com/jinzhu/gorm v1.9.16\n\tgolang.org/x/text v0.3.0\n)\n",
        );
        let deps = declared_dependencies(dir.path());
        assert!(deps.contains("github.com/gin-gonic/gin"));
        assert!(deps.contains("github.com/jinzhu/gorm"));
        assert!(deps.contains("golang.org/x/text"));
    }

    #[test]
    fn pyproject_collects_pep621_and_poetry() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "pyproject.toml",
            "[project]\nname = \"app\"\ndependencies = [\"fastapi>=0.110\", \"sqlalchemy[asyncio]>=2\"]\n\n\
             [tool.poetry.dependencies]\npython = \"^3.11\"\ndjango = \"^5.0\"\n",
        );
        let deps = declared_dependencies(dir.path());
        assert!(deps.contains("fastapi"));
        assert!(deps.contains("sqlalchemy"));
        assert!(deps.contains("django"));
        assert!(deps.contains("python"));
    }

    // -----------------------------------------------------------------------
    // Classification
    // -----------------------------------------------------------------------

    fn sample_doc() -> DependencyVocabularyDoc {
        DependencyVocabularyDoc::parse_str(
            "[[dependency]]\ncategory = \"orm\"\nnames = [\"diesel\", \"sea-orm\"]\n\
             [[dependency]]\ncategory = \"framework\"\nnames = [\"axum\", \"express\"]\n\
             [[dependency]]\ncategory = \"di\"\nprefixes = [\"@nestjs\"]\n",
        )
        .unwrap()
    }

    #[test]
    fn classify_one_matches_exact_name_case_insensitive() {
        let c = DependencyClassifier::from_doc(sample_doc());
        assert_eq!(c.classify_one("Axum"), Some(FrameworkCategory::Framework));
        assert_eq!(c.classify_one("diesel"), Some(FrameworkCategory::Orm));
        assert_eq!(c.classify_one("unknown-lib"), None);
    }

    #[test]
    fn classify_one_matches_prefix() {
        let c = DependencyClassifier::from_doc(sample_doc());
        assert_eq!(
            c.classify_one("@nestjs/common"),
            Some(FrameworkCategory::Di)
        );
    }

    #[test]
    fn classify_set_splits_categories_and_unclassified() {
        let c = DependencyClassifier::from_doc(sample_doc());
        let deps: BTreeSet<String> = ["axum", "diesel", "@nestjs/core", "mystery-crate"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let out = c.classify(&deps);
        assert_eq!(
            out.categories,
            ["di", "framework", "orm"]
                .into_iter()
                .map(str::to_string)
                .collect()
        );
        assert_eq!(
            out.unclassified,
            ["mystery-crate".to_string()].into_iter().collect()
        );
    }

    #[test]
    fn empty_classifier_yields_no_categories_all_unclassified() {
        let c = DependencyClassifier::default();
        assert!(c.is_empty());
        let deps: BTreeSet<String> =
            ["axum", "diesel"].into_iter().map(str::to_string).collect();
        let out = c.classify(&deps);
        assert!(out.categories.is_empty());
        assert_eq!(out.unclassified, deps);
    }

    #[test]
    fn load_absent_vocab_is_empty_not_error() {
        let dir = tempdir().unwrap();
        let c = DependencyClassifier::load(dir.path()).expect("absent vocab is not an error");
        assert!(c.is_empty());
    }

    #[test]
    fn load_present_vocab_drives_classification() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            ".claude/vocab/frameworks.toml",
            "[[dependency]]\ncategory = \"framework\"\nnames = [\"axum\"]\n",
        );
        let c = DependencyClassifier::load(dir.path()).unwrap();
        assert_eq!(c.classify_one("axum"), Some(FrameworkCategory::Framework));
    }

    #[test]
    fn load_ignores_signal_blocks_coexisting_in_file() {
        // The code-pattern `[[signal]]` schema and the dep `[[dependency]]`
        // schema share the file; the dep parser reads only `[[dependency]]`.
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            ".claude/vocab/frameworks.toml",
            "[[signal]]\ncategory = \"orm\"\npatterns = [\"pgTable(\"]\n\n\
             [[dependency]]\ncategory = \"orm\"\nnames = [\"diesel\"]\n",
        );
        let c = DependencyClassifier::load(dir.path()).unwrap();
        assert_eq!(c.classify_one("diesel"), Some(FrameworkCategory::Orm));
    }

    #[test]
    fn malformed_vocab_surfaces_invalid_toml_on_load() {
        let dir = tempdir().unwrap();
        write(dir.path(), ".claude/vocab/frameworks.toml", "this = = not toml");
        let err = DependencyClassifier::load(dir.path()).unwrap_err();
        assert!(matches!(err, VocabError::InvalidToml(_)));
    }

    #[test]
    fn doc_parse_rejects_unknown_category() {
        let err = DependencyVocabularyDoc::parse_str(
            "[[dependency]]\ncategory = \"telepathy\"\nnames = [\"x\"]\n",
        )
        .unwrap_err();
        assert!(matches!(err, VocabError::InvalidToml(_)));
    }

    // -----------------------------------------------------------------------
    // End-to-end: declared dependency + vocab → label; mention-only → no label
    // -----------------------------------------------------------------------

    #[test]
    fn declared_dependency_with_vocab_yields_label() {
        let dir = tempdir().unwrap();
        // A dep DECLARED in the manifest + a vocab mapping it → framework.
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = \"svc\"\n[dependencies]\naxum = \"0.7\"\n",
        );
        write(
            dir.path(),
            ".claude/vocab/frameworks.toml",
            "[[dependency]]\ncategory = \"framework\"\nnames = [\"axum\"]\n",
        );
        let out = classify_subproject_dependencies(dir.path());
        assert!(out.categories.contains("framework"));
    }

    #[test]
    fn source_mention_without_declaration_yields_no_label() {
        let dir = tempdir().unwrap();
        // The dep is NOT declared in the manifest — only mentioned in source.
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = \"lib\"\n[dependencies]\nserde = \"1\"\n",
        );
        write(dir.path(), "src/lib.rs", "// this lib mentions axum in a comment\nuse axum::Router;\n");
        write(
            dir.path(),
            ".claude/vocab/frameworks.toml",
            "[[dependency]]\ncategory = \"framework\"\nnames = [\"axum\"]\n",
        );
        let out = classify_subproject_dependencies(dir.path());
        // `axum` is not a declared dependency, so no framework label — only the
        // declared `serde` surfaces, and it is unclassified by this vocab.
        assert!(!out.categories.contains("framework"));
        assert!(out.unclassified.contains("serde"));
    }
}
