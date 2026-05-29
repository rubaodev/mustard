//! Deterministic structural entity extraction — the **primary** source of the
//! `entity-registry.json` v4 payload (F1-a).
//!
//! Where [`super::interpret`] reaches for an LLM round-trip, this module pulls
//! the same `entities` / `enums` *deterministically* and offline, by:
//!
//! 1. Resolving a [`GrammarLoader::with_builtins`] over the subproject root and
//!    asking [`mustard_core::domain::ast::extract_entities`] for every named
//!    type declaration in each visited file (AST when a grammar resolves,
//!    agnostic textual floor otherwise — never branches on a language id).
//! 2. Reading **fields** (struct/class properties), **enum values**,
//!    **base class**, and **table name** straight from the file text around the
//!    declaration the extractor located (1-indexed line).
//! 3. Tagging **decorators** from [`detect_framework_signals`] (ORM / DI / web
//!    framework markers like `@Entity`, `@Injectable`, `#[derive(Queryable`).
//! 4. Deriving an entity name from an ORM table name via [`super::pluralize`]
//!    (e.g. `pgTable('users', …)` ⇒ `User`) when the file declared no matching
//!    named type itself.
//!
//! The output is the **fountain** the registry builds on: entities discovered
//! here are authoritative and the model-assisted [`super::interpret`] layer may
//! only *add* names this pass never saw — it never overwrites a structural
//! entity (see `sync_entity_registry`). This makes the registry populate fully
//! with no `claude` binary on the PATH.
//!
//! Agnostic by construction: AST when a grammar is in-crate or discovered,
//! textual floor for everything else. Nothing hardcodes a stack.

use super::file_utils::VisitedFile;
use super::{EntityInfo, EnumInfo};
use mustard_core::domain::ast::{extract_entities, ExtractedEntity, GrammarLoader};
use mustard_core::domain::vocabulary::frameworks::{detect_framework_signals, FrameworkCategory};
use std::collections::BTreeMap;
use std::path::Path;

/// The deterministic structural scan of one subproject. Keyed by entity / enum
/// name so the caller merges straight into the byte-stable v4 maps.
#[derive(Debug, Default)]
pub struct StructuralExtraction {
    /// Entities (struct / class / interface / record / type) keyed by name.
    pub entities: BTreeMap<String, EntityInfo>,
    /// Enums keyed by name.
    pub enums: BTreeMap<String, EnumInfo>,
}

/// Whether an AST / floor `kind` token denotes an enum.
///
/// AST mode yields tree-sitter node kinds (`enum_item`, `enum_declaration`);
/// the textual floor yields the keyword that fired (`enum`, `pub enum`,
/// `export enum`, `public enum`). Both contain the substring `enum`.
fn kind_is_enum(kind: &str) -> bool {
    kind.contains("enum")
}

/// Run the structural extraction over the already-visited file set.
///
/// `root` anchors the [`GrammarLoader`] (built-in grammars + any discovered
/// externals). `visited` carries the per-file relative path + cached content
/// the single-pass visitor produced — no extra disk reads happen here.
#[must_use]
pub fn extract(root: &Path, visited: &[VisitedFile]) -> StructuralExtraction {
    let loader = GrammarLoader::with_builtins(root);
    let mut out = StructuralExtraction::default();

    for vf in visited {
        let Some(content) = &vf.content else { continue };
        if content.is_empty() {
            continue;
        }
        let lang_id = loader
            .language_id_for_path(&vf.abs)
            .unwrap_or_default();
        let extracted = extract_entities(&loader, content, &lang_id);
        if extracted.is_empty() {
            // Even with no named declaration the file may still declare an ORM
            // table (e.g. a Drizzle `pgTable('users', …)` const), which yields
            // a structural entity by table-name derivation.
            extract_table_only_entities(content, &vf.rel, &mut out);
            continue;
        }

        // Decorators are a file-level fact (ORM/DI/framework signals), attached
        // to every type the file declares — mirrors how `refs` flows in the AST
        // path (imports are file-level, not per-declaration).
        let signals = detect_framework_signals(content);
        let decorators = decorator_markers(&signals);
        let lines: Vec<&str> = content.lines().collect();

        for ent in &extracted {
            if kind_is_enum(&ent.kind) {
                merge_enum(&mut out, ent, &vf.rel, &lines, &decorators);
            } else {
                merge_entity(&mut out, ent, &vf.rel, &lines, &decorators, content);
            }
        }

        // A file may declare a named type AND an ORM table whose derived name
        // differs (e.g. a Drizzle schema with several `pgTable` consts). Surface
        // any table-derived entity the named-type pass did not already cover.
        extract_table_only_entities(content, &vf.rel, &mut out);
    }

    out
}

/// Merge one extracted enum into the output, reading its member names from the
/// declaration body.
fn merge_enum(
    out: &mut StructuralExtraction,
    ent: &ExtractedEntity,
    rel: &str,
    lines: &[&str],
    decorators: &[String],
) {
    // The structural pass is authoritative: first writer (deterministic visit
    // order) wins; never clobber an already-extracted enum.
    if out.enums.contains_key(&ent.name) {
        return;
    }
    let values = extract_enum_values(lines, ent.line);
    out.enums.insert(
        ent.name.clone(),
        EnumInfo {
            values,
            file: rel.to_string(),
            decorators: decorators.to_vec(),
            value_convention: None,
        },
    );
}

/// Merge one extracted entity into the output, reading its fields / base class
/// / table name from the declaration body.
fn merge_entity(
    out: &mut StructuralExtraction,
    ent: &ExtractedEntity,
    rel: &str,
    lines: &[&str],
    decorators: &[String],
    content: &str,
) {
    if out.entities.contains_key(&ent.name) {
        return;
    }
    let properties = extract_properties(lines, ent.line);
    let base_class = extract_base_class(lines, ent.line);
    // An explicit ORM table declared for *this* named type (`@Table("users")`,
    // `pgTable('users', …)` assigned next to the type) becomes its tableName.
    let table_name = explicit_table_name_for(content, &ent.name);
    // `refs` come from the AST `import_edges` query (file-level imports); empty
    // on the textual floor. Reduce each to its final path segment so the
    // registry's `refs` stay short identifiers, mirroring the cold-path mapping.
    let refs: Vec<String> = ent
        .refs
        .iter()
        .filter_map(|r| ref_tail(r))
        .collect();

    out.entities.insert(
        ent.name.clone(),
        EntityInfo {
            file: rel.to_string(),
            decorators: decorators.to_vec(),
            properties,
            refs,
            sub: Vec::new(),
            enums: Vec::new(),
            base_class,
            table_name,
        },
    );
}

/// Reduce an import edge to a short identifier — the final path / module
/// segment. `crate::foo::Bar` ⇒ `Bar`; `./schema` ⇒ `schema`; `react` ⇒
/// `react`. Returns `None` for empty / punctuation-only inputs.
fn ref_tail(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches(|c| c == '"' || c == '\'' || c == ';');
    let tail = trimmed
        .rsplit(|c| c == ':' || c == '/' || c == '.' || c == '\\')
        .find(|s| !s.is_empty())
        .unwrap_or(trimmed);
    let tail = tail.trim();
    if tail.is_empty() {
        None
    } else {
        Some(tail.to_string())
    }
}

/// Build the decorator marker list from framework signals. ORM / DI markers
/// that begin with `@` (TypeORM/NestJS/Spring/.NET `@Entity`, `@Injectable`, …)
/// or a Rust derive (`#[derive(Queryable`) are class-level decorators; web
/// framework signals are not. Deduplicated, first-seen order.
fn decorator_markers(signals: &[mustard_core::domain::vocabulary::frameworks::FrameworkHit]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for hit in signals {
        let is_decorator = match hit.category {
            FrameworkCategory::Di => true,
            FrameworkCategory::Orm => {
                hit.pattern.starts_with('@') || hit.pattern.starts_with("#[derive(")
            }
            FrameworkCategory::Framework => false,
            // `FrameworkCategory` is `#[non_exhaustive]`; a future category is
            // not a class-level decorator unless it opts in above.
            _ => false,
        };
        if is_decorator && !out.contains(&hit.pattern) {
            out.push(hit.pattern.clone());
        }
    }
    out
}

/// Extract the property / field names declared in the brace block that opens at
/// or just after `decl_line` (1-indexed). Agnostic: collects the leading
/// identifier of each indented `name: Type` / `name Type` / `pub name:` member
/// line until the matching closing brace. Best-effort and tolerant of missing
/// braces (Python-style colon blocks yield nothing here — that is fine, the
/// member harvest is enrichment, not a gate).
fn extract_properties(lines: &[&str], decl_line: usize) -> Vec<String> {
    let body = collect_brace_body(lines, decl_line);
    let mut props: Vec<String> = Vec::new();
    for raw in body {
        let line = raw.trim();
        if line.is_empty() || is_comment(line) {
            continue;
        }
        // Strip common field-visibility / decoration prefixes so the leading
        // token is the field shape, not a modifier.
        let mut rest = line;
        loop {
            let mut advanced = false;
            for prefix in [
                "pub(crate) ", "pub ", "public ", "private ", "protected ", "readonly ", "final ",
                "static ", "const ",
            ] {
                if let Some(stripped) = rest.strip_prefix(prefix) {
                    rest = stripped.trim_start();
                    advanced = true;
                }
            }
            if !advanced {
                break;
            }
        }
        let Some(name) = field_name(rest) else { continue };
        if !props.contains(&name) {
            props.push(name);
        }
    }
    props
}

/// Resolve the field *name* from a (modifier-stripped) member line, agnostic
/// to declaration order:
///
/// - **Colon form** (`name: Type`, Rust/TS/Swift/Kotlin/typed-Python): the
///   leading identifier before the first `:`.
/// - **Space form** (`Type name;` C#/Java, `name = …` assignment): the last
///   identifier before a terminator (`;`, `=`, `{`, `(`). This makes
///   `public int Id;` ⇒ `Id`.
///
/// Returns `None` for method lines (`name(`), empty lines, or lines with no
/// identifier.
fn field_name(rest: &str) -> Option<String> {
    let lead = read_ident(rest);
    if lead.is_empty() {
        return None;
    }
    let after_lead = rest[lead.len().min(rest.len())..].trim_start();
    // Method declaration — not a field.
    if after_lead.starts_with('(') {
        return None;
    }
    // Colon form: the leading identifier is the field name.
    if after_lead.starts_with(':') {
        return Some(lead);
    }
    // Space / declaration form: take the last identifier before a terminator.
    // Truncate at the first terminator so trailing `= default(...)` / `{ get; }`
    // do not leak in.
    let head: &str = rest
        .split(['=', ';', '{', '('])
        .next()
        .unwrap_or(rest);
    let last = head
        .split_whitespace()
        .filter_map(|tok| {
            let id = read_ident(tok);
            if id.is_empty() { None } else { Some(id) }
        })
        .next_back();
    last
}

/// Extract enum member names from the brace block opened at/after `decl_line`.
/// Each non-empty, non-comment body line's leading identifier is a variant
/// (Rust `Active,` / TS `Admin = 'admin'` / Java `PENDING`).
fn extract_enum_values(lines: &[&str], decl_line: usize) -> Vec<String> {
    let body = collect_brace_body(lines, decl_line);
    let mut values: Vec<String> = Vec::new();
    for raw in body {
        let line = raw.trim();
        if line.is_empty() || is_comment(line) {
            continue;
        }
        let ident = read_ident(line);
        if ident.is_empty() {
            continue;
        }
        // Reject method-like / impl lines that can sneak into a malformed body.
        let after = line[ident.len()..].trim_start();
        if after.starts_with('(') {
            continue;
        }
        if !values.contains(&ident) {
            values.push(ident);
        }
    }
    values
}

/// Collect the lines inside the first `{ … }` block at or after `decl_line`
/// (1-indexed), tracking brace depth so nested braces do not close early.
/// Returns the inner lines (excluding the brace lines themselves). When no
/// opening brace is found within a small look-ahead window, returns empty.
fn collect_brace_body<'a>(lines: &[&'a str], decl_line: usize) -> Vec<&'a str> {
    if decl_line == 0 || decl_line > lines.len() {
        return Vec::new();
    }
    // Find the opening brace, scanning from the declaration line forward a few
    // lines (the brace may sit on the same line or the next for `{`-on-newline
    // styles).
    let start_idx = decl_line - 1;
    let mut open_idx = None;
    let mut open_col = 0usize;
    'outer: for (offset, line) in lines.iter().enumerate().skip(start_idx).take(8) {
        if let Some(col) = line.find('{') {
            open_idx = Some(offset);
            open_col = col;
            break 'outer;
        }
    }
    let Some(open_idx) = open_idx else {
        return Vec::new();
    };

    let mut body: Vec<&str> = Vec::new();
    let mut depth = 0i32;
    for (offset, line) in lines.iter().enumerate().skip(open_idx) {
        // Push the line as a body line when we are strictly inside (depth >= 1)
        // and it is not the opening brace line itself. Captured before this
        // line's braces are counted so the closing-brace line is excluded.
        if offset > open_idx && depth >= 1 {
            body.push(line);
        }
        // Count braces on this line to maintain depth; the entity's own block
        // closes when depth returns to 0.
        let scan_from = if offset == open_idx { open_col } else { 0 };
        for ch in line[scan_from..].chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return body;
                    }
                }
                _ => {}
            }
        }
    }
    body
}

/// Extract a base class / superclass for the declaration at `decl_line`.
///
/// Recognises the universal inheritance markers on the declaration line:
/// `extends Base` (TS/Java/JS), `: Base` (C#) — the first type after the
/// marker. Rust/Go have no class inheritance, so this yields `None` there.
fn extract_base_class(lines: &[&str], decl_line: usize) -> Option<String> {
    if decl_line == 0 || decl_line > lines.len() {
        return None;
    }
    let line = lines[decl_line - 1];
    // `extends Base` — take the identifier after the keyword.
    if let Some(idx) = line.find(" extends ") {
        let after = line[idx + " extends ".len()..].trim_start();
        let base = read_dotted_ident(after);
        let bare = base.rsplit('.').next().unwrap_or(&base);
        if !bare.is_empty() {
            return Some(bare.to_string());
        }
    }
    // C# `class Foo : Base` — the segment after a top-level `:` that is not a
    // generic constraint. Guard against TS `name: Type` field lines by only
    // honouring `:` that appears after a `class`/`record`/`struct`/`interface`.
    if let Some(colon) = line.find(':') {
        let before = &line[..colon];
        let is_type_decl = ["class ", "record ", "struct ", "interface "]
            .iter()
            .any(|kw| before.contains(kw));
        if is_type_decl {
            let after = line[colon + 1..].trim_start();
            let base = read_dotted_ident(after);
            let bare = base.rsplit('.').next().unwrap_or(&base);
            if !bare.is_empty() {
                return Some(bare.to_string());
            }
        }
    }
    None
}

/// Find an explicit ORM table name *for a specific named type* in `content`.
///
/// Recognises:
/// - `@Table("users")` / `@Table(name = "users")` attribute on a class.
/// - `pgTable('users', …)` / `mysqlTable(…)` / `sqliteTable(…)` const whose
///   variable basename matches `entity_name` case-insensitively (Drizzle).
///
/// Returns the literal table name when one is unambiguously attached to the
/// entity. `None` otherwise — we never guess.
fn explicit_table_name_for(content: &str, entity_name: &str) -> Option<String> {
    // `@Table(...)` — TypeORM / JPA / .NET. Take the first quoted string in the
    // arg list (works for both `@Table("users")` and `@Table(name = "users")`).
    if let Some(idx) = content.find("@Table(") {
        let after = &content[idx + "@Table(".len()..];
        if let Some(name) = first_quoted(after) {
            return Some(name);
        }
    }
    // Drizzle `const <var> = pgTable('users', …)`. Match the table-constructor
    // call and require the const variable's PascalCase to relate to the entity.
    for ctor in ["pgTable(", "mysqlTable(", "sqliteTable("] {
        let mut search = 0;
        while let Some(rel) = content[search..].find(ctor) {
            let idx = search + rel;
            search = idx + ctor.len();
            let after = &content[idx + ctor.len()..];
            let Some(table) = first_quoted(after) else { continue };
            // The named type is typically the singular PascalCase of the table.
            let derived = super::pluralize::snake_to_pascal_singular(&table);
            if derived.eq_ignore_ascii_case(entity_name) {
                return Some(table);
            }
        }
    }
    None
}

/// Harvest entities that exist **only** as ORM table declarations (no named
/// type the AST/floor would catch), deriving a PascalCase singular entity name
/// from the table name via [`super::pluralize`]. Drizzle schemas are the
/// canonical case: `export const users = pgTable('users', {...})` declares an
/// `User` entity with no `class`/`struct`.
fn extract_table_only_entities(content: &str, rel: &str, out: &mut StructuralExtraction) {
    for ctor in ["pgTable(", "mysqlTable(", "sqliteTable("] {
        let mut search = 0;
        while let Some(rel_idx) = content[search..].find(ctor) {
            let idx = search + rel_idx;
            search = idx + ctor.len();
            let after = &content[idx + ctor.len()..];
            let Some(table) = first_quoted(after) else { continue };
            let name = super::pluralize::snake_to_pascal_singular(&table);
            if name.is_empty() || out.entities.contains_key(&name) {
                continue;
            }
            // Pull field names from the object literal that follows the table
            // name argument (the second `{ … }` block of the constructor call).
            let properties = extract_drizzle_columns(after);
            out.entities.insert(
                name,
                EntityInfo {
                    file: rel.to_string(),
                    properties,
                    table_name: Some(table),
                    ..EntityInfo::default()
                },
            );
        }
    }
}

/// Extract column names from a Drizzle table-constructor tail
/// (`'users', { id: serial(), name: text() })`). Reads the leading identifier
/// of each `key: value` pair inside the object-literal block.
fn extract_drizzle_columns(after_ctor: &str) -> Vec<String> {
    // Find the object literal (first `{` after the table-name string arg).
    let Some(brace) = after_ctor.find('{') else {
        return Vec::new();
    };
    let mut cols: Vec<String> = Vec::new();
    let mut depth = 0i32;
    let body = &after_ctor[brace..];
    for segment in body.split([',', '\n']) {
        let seg = segment.trim();
        // Inspect each comma/newline-delimited entry. A column key is the
        // leading `ident:` of an entry while we are at the table object's own
        // depth (1) — nested object args (`text().notNull()`) do not start a
        // new key. Depth is checked *before* counting this segment's braces so a
        // key on the same line as the opening `{` still registers.
        let entry_depth = depth;
        for ch in seg.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if entry_depth <= 1 {
            let key_src = seg.trim_start_matches('{').trim_start();
            let ident = read_ident(key_src);
            if !ident.is_empty()
                && key_src[ident.len()..].trim_start().starts_with(':')
                && !cols.contains(&ident)
            {
                cols.push(ident);
            }
        }
    }
    cols
}

/// First single- or double-quoted string literal in `s`, unquoted. `None` when
/// no quoted literal is present before the next `)`.
fn first_quoted(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == ')' {
            return None;
        }
        if c == '\'' || c == '"' {
            let quote = c;
            let rest = &s[i + 1..];
            if let Some(end) = rest.find(quote) {
                let lit = &rest[..end];
                if !lit.is_empty() {
                    return Some(lit.to_string());
                }
            }
            return None;
        }
        i += 1;
    }
    None
}

/// Read the leading `[A-Za-z_][A-Za-z0-9_]*` identifier of `s` (empty when none).
fn read_ident(s: &str) -> String {
    let s = s.trim_start().trim_start_matches(['*', '&']).trim_start();
    s.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

/// Read a possibly-dotted identifier (`pkg.Base`) for base-class resolution.
fn read_dotted_ident(s: &str) -> String {
    s.trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
        .collect()
}

/// Universal single-line comment check for a trimmed line.
fn is_comment(line: &str) -> bool {
    const PREFIXES: &[&str] = &["//", "#", "--", ";", "%", "/*", "*"];
    PREFIXES.iter().any(|p| line.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn vf(rel: &str, content: &str) -> VisitedFile {
        VisitedFile {
            abs: PathBuf::from(rel),
            rel: rel.to_string(),
            content: Some(content.to_string()),
        }
    }

    #[test]
    fn rust_struct_and_enum_extracted_without_llm() {
        let tmp = tempfile::tempdir().unwrap();
        let visited = vec![vf(
            "src/models.rs",
            "pub struct User {\n    pub id: i32,\n    pub name: String,\n}\n\n\
             pub enum Status {\n    Active,\n    Pending,\n}\n",
        )];
        let out = extract(tmp.path(), &visited);
        let user = out.entities.get("User").expect("User entity");
        assert_eq!(user.file, "src/models.rs");
        assert!(user.properties.contains(&"id".to_string()));
        assert!(user.properties.contains(&"name".to_string()));
        let status = out.enums.get("Status").expect("Status enum");
        assert!(status.values.contains(&"Active".to_string()));
        assert!(status.values.contains(&"Pending".to_string()));
    }

    #[test]
    fn csharp_class_with_base_class() {
        let tmp = tempfile::tempdir().unwrap();
        let visited = vec![vf(
            "Order.cs",
            "public class Order : BaseEntity {\n    public int Id;\n    public string Name;\n}\n",
        )];
        let out = extract(tmp.path(), &visited);
        let order = out.entities.get("Order").expect("Order entity");
        assert_eq!(order.base_class.as_deref(), Some("BaseEntity"));
        assert!(order.properties.contains(&"Id".to_string()));
    }

    #[test]
    fn typescript_extends_base() {
        let tmp = tempfile::tempdir().unwrap();
        let visited = vec![vf(
            "widget.ts",
            "export class Widget extends Base {\n  id: number;\n}\n",
        )];
        let out = extract(tmp.path(), &visited);
        let w = out.entities.get("Widget").expect("Widget entity");
        assert_eq!(w.base_class.as_deref(), Some("Base"));
    }

    #[test]
    fn drizzle_table_derives_singular_entity() {
        let tmp = tempfile::tempdir().unwrap();
        let visited = vec![vf(
            "schema.ts",
            "export const users = pgTable('users', {\n  id: serial(),\n  name: text(),\n});\n",
        )];
        let out = extract(tmp.path(), &visited);
        let user = out.entities.get("User").expect("User entity from table");
        assert_eq!(user.table_name.as_deref(), Some("users"));
        assert!(user.properties.contains(&"id".to_string()));
        assert!(user.properties.contains(&"name".to_string()));
    }

    #[test]
    fn go_struct_via_floor_or_ast() {
        // Go's `type Customer struct { Name string }` is positional (name first,
        // type second) — the language-agnostic field heuristic cannot tell which
        // token is the name without the grammar's field nodes, so we assert only
        // the load-bearing fact: the named type is recovered. Properties are
        // best-effort enrichment, not a gate.
        let tmp = tempfile::tempdir().unwrap();
        let visited = vec![vf(
            "customer.go",
            "package internal\n\ntype Customer struct {\n    Name string\n    Age  int\n}\n",
        )];
        let out = extract(tmp.path(), &visited);
        assert!(out.entities.contains_key("Customer"), "got {:?}", out.entities);
    }

    #[test]
    fn exotic_extension_uses_textual_floor() {
        // A made-up `.zig`-like file with no in-crate grammar must still yield
        // the named type via the agnostic floor.
        let tmp = tempfile::tempdir().unwrap();
        let visited = vec![vf(
            "thing.exotic",
            "struct Thing {\n    field: u8,\n}\n",
        )];
        let out = extract(tmp.path(), &visited);
        assert!(out.entities.contains_key("Thing"), "got {:?}", out.entities);
    }

    #[test]
    fn typeorm_entity_decorator_captured() {
        let tmp = tempfile::tempdir().unwrap();
        let visited = vec![vf(
            "order.ts",
            "@Entity\n@Table(\"orders\")\nexport class Order {\n  id: number;\n}\n",
        )];
        let out = extract(tmp.path(), &visited);
        let order = out.entities.get("Order").expect("Order entity");
        assert!(order.decorators.iter().any(|d| d == "@Entity"));
        assert_eq!(order.table_name.as_deref(), Some("orders"));
    }

    #[test]
    fn empty_visited_yields_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let out = extract(tmp.path(), &[]);
        assert!(out.entities.is_empty());
        assert!(out.enums.is_empty());
    }
}
