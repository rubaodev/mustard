//! `frameworks` — deterministic, language-agnostic detection of
//! framework / ORM / DI-decorator signals in source content.
//!
//! Where the regression vocabulary ([`super::VocabularyMatcher`]) ranks intent
//! drift by severity layer, this module answers a different question: *what
//! stack does this file belong to?* — by scanning code content for literal
//! signals (`pgTable(`, `@Entity`, `#[derive(Queryable`, `DbSet<`,
//! `@SpringBootApplication`, …) and reporting which [`FrameworkCategory`] each
//! match belongs to. No LLM, no regex alternation: one Aho-Corasick pass over
//! the content, reusing the shared [`super::aho::KeyedAutomaton`] engine.
//!
//! ## Why a small schema instead of reusing `Layer`
//!
//! `Layer` is the *closed* four-value severity scale the regression gate is
//! built on (semantic > pattern > keyword > noise). Framework categories are a
//! different, orthogonal axis (ORM / framework / DI) with no severity ordering.
//! Overloading `Layer` with framework meaning would conflate two unrelated
//! concerns and break the gate's invariant. So this module defines its own
//! tiny [`FrameworkCategory`] / [`FrameworkSignal`] schema — but it does **not**
//! reimplement the Aho engine: both matchers feed [`KeyedAutomaton`].
//!
//! ## Built-in base + on-disk override
//!
//! The base vocabulary is embedded via [`include_str!`] from
//! `frameworks_builtin.toml`, so detection works offline. A project may
//! override it by dropping `.claude/vocab/{name}.toml` (default name
//! `frameworks`); when that file exists it **replaces** the built-in base
//! wholesale — see [`FrameworkVocabulary::load`]. This mirrors the
//! built-in-plus-override policy `ast::QuerySet` already ships.
//!
//! [`KeyedAutomaton`]: super::aho::KeyedAutomaton

use super::aho::KeyedAutomaton;
use super::VocabError;
use serde::Deserialize;
use std::path::Path;

/// The built-in framework vocabulary, embedded at compile time. The
/// *guaranteed base* for [`detect_framework_signals`]; an on-disk vocab
/// overrides it (see [`FrameworkVocabulary::load`]).
const BUILTIN_FRAMEWORKS_TOML: &str = include_str!("frameworks_builtin.toml");

/// The default on-disk vocabulary name resolved under `.claude/vocab/`.
/// [`FrameworkVocabulary::load`] looks for `.claude/vocab/frameworks.toml`.
pub const DEFAULT_FRAMEWORKS_NAME: &str = "frameworks";

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

/// The category a framework signal belongs to. An *open* taxonomy axis,
/// orthogonal to [`super::Layer`]: there is no severity ordering between
/// categories. New categories are added to the TOML and this enum together.
///
/// `#[non_exhaustive]` so a later wave can add a variant (e.g. `Migration`,
/// `Validation`) without breaking a downstream `match`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum FrameworkCategory {
    /// Object/relational mapping signals: `pgTable(`, `@Entity`,
    /// `#[derive(Queryable`, `DbSet<`, `gorm.Model`, `SQLAlchemy`, …
    Orm,
    /// Web / application framework signals: `axum::`, `FastAPI(`, `express(`,
    /// `@SpringBootApplication`, `[ApiController]`, `gin.`, …
    Framework,
    /// Dependency-injection / decorator signals: `@Injectable`, `@Component`,
    /// `@Autowired`, …
    Di,
}

impl FrameworkCategory {
    /// Canonical lowercase name used in the TOML `category = "..."` field and
    /// returned to F1 consumers as the architecture-field tag.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Orm => "orm",
            Self::Framework => "framework",
            Self::Di => "di",
        }
    }
}

/// One category group in a framework vocabulary — a category plus its literal
/// signal patterns. The TOML representation is one `[[signal]]` table-array
/// entry per instance.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FrameworkSignal {
    /// Which category these patterns belong to. Closed enum — an unknown
    /// `category` value surfaces as [`VocabError::InvalidToml`].
    pub category: FrameworkCategory,
    /// The literal substrings to match. Empty strings are dropped by the
    /// matcher constructor; duplicates (within and across categories) collapse.
    #[serde(default)]
    pub patterns: Vec<String>,
}

/// Top-level document deserialised from a framework vocabulary TOML. The
/// `[[signal]]` table array is the only key.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct FrameworkVocabularyDoc {
    /// Every `[[signal]]` table entry, in document order (= priority order).
    #[serde(default, rename = "signal")]
    pub signals: Vec<FrameworkSignal>,
}

impl FrameworkVocabularyDoc {
    /// Parse a framework vocabulary TOML document. Pure on `&str`.
    ///
    /// # Errors
    /// Returns [`VocabError::InvalidToml`] when the input cannot be
    /// deserialised (bad `category`, malformed table array, …).
    pub fn parse_str(raw: &str) -> Result<Self, VocabError> {
        toml::from_str::<Self>(raw).map_err(|e| VocabError::InvalidToml(e.to_string()))
    }
}

/// One signal matched by [`FrameworkVocabulary::detect`] /
/// [`detect_framework_signals`]: the matched pattern, its category, and the
/// 1-based line it starts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameworkHit {
    /// The signal pattern as it appears in the vocabulary (NOT a substring of
    /// the haystack — relevant when the haystack contains case variants).
    pub pattern: String,
    /// Which category the matched pattern belongs to.
    pub category: FrameworkCategory,
    /// Byte offset where the match starts (inclusive).
    pub start: usize,
    /// Byte offset where the match ends (exclusive).
    pub end: usize,
    /// 1-based line number the match starts on. Computed by counting newlines
    /// up to [`Self::start`]; cheap for the per-file content this scans.
    pub line: usize,
}

// ---------------------------------------------------------------------------
// Matcher
// ---------------------------------------------------------------------------

/// A built framework-signal matcher: the shared Aho-Corasick engine keyed on
/// [`FrameworkCategory`]. Construct via [`FrameworkVocabulary::builtin`],
/// [`FrameworkVocabulary::from_doc`], or [`FrameworkVocabulary::load`].
pub struct FrameworkVocabulary {
    inner: KeyedAutomaton<FrameworkCategory>,
}

impl std::fmt::Debug for FrameworkVocabulary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameworkVocabulary")
            .field("signal_count", &self.inner.term_count())
            .finish()
    }
}

impl FrameworkVocabulary {
    /// Build a matcher from a parsed document. Signals are fed to the shared
    /// engine in document order so the first listed category wins a
    /// cross-category pattern collision.
    ///
    /// # Errors
    /// Returns [`VocabError::NoTerms`] when no non-empty pattern survives
    /// across every category.
    pub fn from_doc(doc: FrameworkVocabularyDoc) -> Result<Self, VocabError> {
        let groups = doc.signals.into_iter().map(|s| (s.category, s.patterns));
        Ok(Self {
            inner: KeyedAutomaton::from_groups(groups)?,
        })
    }

    /// Build the matcher from the embedded built-in vocabulary. Infallible in
    /// practice (the embedded TOML is validated by a unit test), but the
    /// constructor still surfaces a typed error rather than panicking so the
    /// `unwrap_used = deny` contract holds at every call site.
    ///
    /// # Errors
    /// Returns [`VocabError::InvalidToml`] if the embedded TOML ever fails to
    /// parse, or [`VocabError::NoTerms`] if it is emptied.
    pub fn builtin() -> Result<Self, VocabError> {
        let doc = FrameworkVocabularyDoc::parse_str(BUILTIN_FRAMEWORKS_TOML)?;
        Self::from_doc(doc)
    }

    /// Load a *named* framework vocabulary, preferring an on-disk override over
    /// the built-in base.
    ///
    /// Resolution:
    /// 1. If `{project_root}/.claude/vocab/{name}.toml` exists and parses, it
    ///    **replaces** the built-in base wholesale and is used as-is.
    /// 2. Otherwise (file absent) the embedded built-in base is used — but
    ///    only when `name` is [`DEFAULT_FRAMEWORKS_NAME`]; a named vocab that
    ///    does not exist on disk is a [`VocabError::FileNotFound`], because
    ///    silently substituting the framework base for an unrelated named
    ///    vocab would hide a misconfiguration.
    ///
    /// A file that exists but fails to parse surfaces the parse error (it is
    /// *not* fail-open to the built-in): a malformed override is a real
    /// configuration bug the caller should see.
    ///
    /// # Errors
    /// [`VocabError::FileNotFound`] (named vocab absent, non-default name),
    /// [`VocabError::InvalidToml`] (override present but unparseable),
    /// [`VocabError::Io`] (read failure), or [`VocabError::NoTerms`].
    pub fn load(name: &str, project_root: &Path) -> Result<Self, VocabError> {
        let path = project_root
            .join(".claude")
            .join("vocab")
            .join(format!("{name}.toml"));

        match std::fs::read_to_string(&path) {
            Ok(raw) => {
                let doc = FrameworkVocabularyDoc::parse_str(&raw)?;
                Self::from_doc(doc)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if name == DEFAULT_FRAMEWORKS_NAME {
                    Self::builtin()
                } else {
                    Err(VocabError::FileNotFound(path.display().to_string()))
                }
            }
            Err(e) => Err(VocabError::Io(e.to_string())),
        }
    }

    /// Scan `content` and return every framework signal it contains, in
    /// left-to-right order. O(n + m) — one Aho-Corasick pass.
    #[must_use]
    pub fn detect(&self, content: &str) -> Vec<FrameworkHit> {
        self.inner
            .scan(content)
            .into_iter()
            .map(|h| FrameworkHit {
                pattern: h.term,
                category: h.key,
                start: h.start,
                end: h.end,
                line: line_of(content, h.start),
            })
            .collect()
    }

    /// Total number of distinct signal patterns across every category.
    #[must_use]
    pub fn signal_count(&self) -> usize {
        self.inner.term_count()
    }

    /// Number of distinct signal patterns in one category.
    #[must_use]
    pub fn signal_count_for(&self, category: FrameworkCategory) -> usize {
        self.inner.term_count_for(category)
    }
}

/// Detect framework / ORM / DI signals in `content` using the **built-in**
/// vocabulary. Convenience entry point for callers that do not need a
/// project-local override; equivalent to building [`FrameworkVocabulary::builtin`]
/// and calling [`FrameworkVocabulary::detect`].
///
/// Returns an empty `Vec` when no signal matches, or when the built-in
/// vocabulary ever fails to build (fail-open: detection degrades to "no
/// signals", never panics). Callers that need to distinguish "no signals" from
/// "matcher build error", or that want a project-local override, should build a
/// [`FrameworkVocabulary`] explicitly and call [`FrameworkVocabulary::detect`].
#[must_use]
pub fn detect_framework_signals(content: &str) -> Vec<FrameworkHit> {
    match FrameworkVocabulary::builtin() {
        Ok(v) => v.detect(content),
        Err(_) => Vec::new(),
    }
}

/// Detect framework / ORM / DI signals in `content`, honouring a project-local
/// vocabulary override. Resolves the matcher via
/// [`FrameworkVocabulary::load`]`(`[`DEFAULT_FRAMEWORKS_NAME`]`, root)` so a
/// `.claude/vocab/frameworks.toml` under `root` **replaces** the built-in base,
/// while a project with no override falls back to it.
///
/// This is the override-aware sibling of [`detect_framework_signals`] (which is
/// pinned to the built-in base). Returns an empty `Vec` when no signal matches,
/// or when the resolved vocabulary fails to build (fail-open: detection degrades
/// to "no signals", never panics — a malformed override yields nothing here
/// rather than surfacing the error, matching the scan pipeline's fail-open
/// stance; callers that need to distinguish the two should call
/// [`FrameworkVocabulary::load`] directly).
#[must_use]
pub fn detect_framework_signals_with(root: &Path, content: &str) -> Vec<FrameworkHit> {
    match FrameworkVocabulary::load(DEFAULT_FRAMEWORKS_NAME, root) {
        Ok(v) => v.detect(content),
        Err(_) => Vec::new(),
    }
}

/// 1-based line number of byte offset `at` in `content` (count of newlines
/// before `at`, plus one). Saturates rather than panicking on an out-of-range
/// offset.
fn line_of(content: &str, at: usize) -> usize {
    let upto = at.min(content.len());
    content[..upto].bytes().filter(|&b| b == b'\n').count() + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Built-in vocabulary
    // -----------------------------------------------------------------------

    #[test]
    fn builtin_toml_parses_and_builds() {
        let v = FrameworkVocabulary::builtin().expect("built-in framework vocab builds");
        assert!(v.signal_count() > 0);
        // Every category in the built-in base has at least one signal.
        assert!(v.signal_count_for(FrameworkCategory::Orm) > 0);
        assert!(v.signal_count_for(FrameworkCategory::Framework) > 0);
        assert!(v.signal_count_for(FrameworkCategory::Di) > 0);
    }

    #[test]
    fn category_round_trips_through_as_str() {
        for (raw, cat) in [
            ("orm", FrameworkCategory::Orm),
            ("framework", FrameworkCategory::Framework),
            ("di", FrameworkCategory::Di),
        ] {
            assert_eq!(cat.as_str(), raw);
        }
    }

    #[test]
    fn doc_parse_rejects_unknown_category() {
        let toml = r#"
[[signal]]
category = "telepathy"
patterns = ["x"]
"#;
        let err = FrameworkVocabularyDoc::parse_str(toml).unwrap_err();
        assert!(matches!(err, VocabError::InvalidToml(_)));
    }

    // -----------------------------------------------------------------------
    // detect_framework_signals — the required acceptance cases
    // -----------------------------------------------------------------------

    fn categories_for(content: &str) -> Vec<FrameworkCategory> {
        detect_framework_signals(content)
            .into_iter()
            .map(|h| h.category)
            .collect()
    }

    #[test]
    fn detects_drizzle_pg_table_as_orm() {
        let hits = detect_framework_signals("export const users = pgTable('users', {...})");
        let hit = hits
            .iter()
            .find(|h| h.pattern == "pgTable(")
            .expect("pgTable( matched");
        assert_eq!(hit.category, FrameworkCategory::Orm);
        assert_eq!(hit.line, 1);
    }

    #[test]
    fn detects_diesel_derive_queryable_as_orm() {
        let cats = categories_for("#[derive(Queryable)] pub struct User;");
        assert!(cats.contains(&FrameworkCategory::Orm));
    }

    #[test]
    fn detects_injectable_decorator_as_di() {
        let cats = categories_for("@Injectable() export class Svc {}");
        assert!(cats.contains(&FrameworkCategory::Di));
    }

    #[test]
    fn detects_dbcontext_as_orm() {
        let cats = categories_for("public class C : DbContext {}");
        assert!(cats.contains(&FrameworkCategory::Orm));
    }

    #[test]
    fn detects_dbset_as_orm() {
        let cats = categories_for("public DbSet<User> Users { get; set; }");
        assert!(cats.contains(&FrameworkCategory::Orm));
    }

    #[test]
    fn returns_empty_for_content_without_signals() {
        let hits = detect_framework_signals("fn add(a: i32, b: i32) -> i32 { a + b }");
        assert!(hits.is_empty());
    }

    // -----------------------------------------------------------------------
    // Spans / lines
    // -----------------------------------------------------------------------

    #[test]
    fn hit_span_points_at_the_pattern() {
        let content = "let app = FastAPI()";
        let hits = detect_framework_signals(content);
        let hit = hits.iter().find(|h| h.pattern == "FastAPI(").unwrap();
        assert_eq!(&content[hit.start..hit.end], "FastAPI(");
        assert_eq!(hit.category, FrameworkCategory::Framework);
    }

    #[test]
    fn line_number_tracks_newlines() {
        let content = "// header\n// more\n@Injectable()\nclass S {}";
        let hits = detect_framework_signals(content);
        let hit = hits.iter().find(|h| h.pattern == "@Injectable").unwrap();
        assert_eq!(hit.line, 3);
    }

    // -----------------------------------------------------------------------
    // Named load + on-disk override
    // -----------------------------------------------------------------------

    #[test]
    fn load_default_falls_back_to_builtin_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let v = FrameworkVocabulary::load(DEFAULT_FRAMEWORKS_NAME, tmp.path())
            .expect("default name falls back to built-in");
        // Built-in base detects pgTable.
        let hits = v.detect("x = pgTable(");
        assert!(hits.iter().any(|h| h.pattern == "pgTable("));
    }

    #[test]
    fn load_non_default_named_vocab_errors_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let err = FrameworkVocabulary::load("custom-stack", tmp.path()).unwrap_err();
        assert!(matches!(err, VocabError::FileNotFound(_)));
    }

    #[test]
    fn on_disk_override_replaces_builtin() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".claude").join("vocab");
        std::fs::create_dir_all(&dir).unwrap();
        // An override that only knows about a bespoke ORM signal.
        std::fs::write(
            dir.join("frameworks.toml"),
            r#"
[[signal]]
category = "orm"
patterns = ["MyCustomEntity("]
"#,
        )
        .unwrap();

        let v = FrameworkVocabulary::load(DEFAULT_FRAMEWORKS_NAME, tmp.path()).unwrap();
        // The override IS respected: its bespoke signal matches...
        let custom = v.detect("x = MyCustomEntity()");
        assert!(custom.iter().any(|h| h.pattern == "MyCustomEntity("));
        // ...and the built-in base is fully replaced: pgTable no longer matches.
        let builtin = v.detect("x = pgTable(");
        assert!(builtin.is_empty());
    }

    #[test]
    fn detect_with_falls_back_to_builtin_when_no_override() {
        let tmp = tempfile::tempdir().unwrap();
        // No `.claude/vocab/frameworks.toml` ⇒ the built-in base answers.
        let hits = detect_framework_signals_with(tmp.path(), "x = pgTable(");
        assert!(hits.iter().any(|h| h.pattern == "pgTable("));
    }

    #[test]
    fn detect_with_honours_on_disk_override() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".claude").join("vocab");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("frameworks.toml"),
            r#"
[[signal]]
category = "orm"
patterns = ["MyCustomEntity("]
"#,
        )
        .unwrap();
        // The override IS honoured: its bespoke signal matches...
        let custom = detect_framework_signals_with(tmp.path(), "x = MyCustomEntity()");
        assert!(custom.iter().any(|h| h.pattern == "MyCustomEntity("));
        // ...and the built-in base is fully replaced: pgTable no longer matches.
        let builtin = detect_framework_signals_with(tmp.path(), "x = pgTable(");
        assert!(builtin.is_empty());
    }

    #[test]
    fn detect_with_is_fail_open_on_malformed_override() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".claude").join("vocab");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("frameworks.toml"), "this is = = not toml").unwrap();
        // Unlike `load`, the scan-facing helper degrades to "no signals".
        assert!(detect_framework_signals_with(tmp.path(), "x = pgTable(").is_empty());
    }

    #[test]
    fn on_disk_override_parse_error_is_surfaced() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".claude").join("vocab");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("frameworks.toml"), "this is = = not toml").unwrap();
        let err = FrameworkVocabulary::load(DEFAULT_FRAMEWORKS_NAME, tmp.path()).unwrap_err();
        assert!(matches!(err, VocabError::InvalidToml(_)));
    }

    #[test]
    fn cross_category_collision_keeps_first_listed() {
        // Two categories share a term; document order (orm first) wins.
        let toml = r#"
[[signal]]
category = "orm"
patterns = ["@Shared"]

[[signal]]
category = "di"
patterns = ["@Shared"]
"#;
        let doc = FrameworkVocabularyDoc::parse_str(toml).unwrap();
        let v = FrameworkVocabulary::from_doc(doc).unwrap();
        let hits = v.detect("@Shared here");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].category, FrameworkCategory::Orm);
    }
}
