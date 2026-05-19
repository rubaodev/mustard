//! TypeScript / Node.js stack scanner — a port of
//! `registry/scanners/typescript-scanner.js`.
//!
//! Detects Drizzle, Prisma and TypeORM entities plus TypeScript enums. The JS
//! scanner used regular expressions; the extraction here is rewritten with
//! hand-written string scanning that preserves the same decision logic.

use super::file_utils::{collect_files, read_file_safe, relative_path};
use super::rust_scanner::extract_brace_body;
use super::{detect_value_convention, EntityInfo, EnumInfo, Scanner};
use std::collections::BTreeMap;
use std::path::Path;

/// TypeScript scanner — selected when `package.json` / `tsconfig.json` is present.
pub struct TypeScriptScanner;

/// Frameworks detected from `package.json` dependency keys.
struct Frameworks {
    drizzle: bool,
    prisma: bool,
    typeorm: bool,
}

/// `true` if `root` has a `package.json` declaring `dep` as a (dev)dependency.
fn package_has_dep(root: &Path, dep: &str) -> bool {
    let Some(content) = read_file_safe(&root.join("package.json")) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return false;
    };
    let in_section = |section: &str| {
        json.get(section)
            .and_then(serde_json::Value::as_object)
            .is_some_and(|obj| obj.contains_key(dep))
    };
    in_section("dependencies") || in_section("devDependencies")
}

/// Uppercase the first ASCII character — a port of `_toPascalCase`.
fn to_pascal_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => name.to_string(),
        Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

impl TypeScriptScanner {
    fn detect_frameworks(root: &Path) -> Frameworks {
        Frameworks {
            drizzle: package_has_dep(root, "drizzle-orm") || package_has_dep(root, "drizzle-kit"),
            prisma: package_has_dep(root, "@prisma/client")
                || package_has_dep(root, "prisma")
                || root.join("prisma/schema.prisma").exists(),
            typeorm: package_has_dep(root, "typeorm"),
        }
    }

    /// Drizzle: `export const tableVar = pgTable('table_name', { … })`.
    fn scan_drizzle(root: &Path, entities: &mut BTreeMap<String, EntityInfo>) {
        for file in collect_files(root, ".ts", &[]) {
            let Some(content) = read_file_safe(&file) else {
                continue;
            };
            if !content.contains("pgTable") {
                continue;
            }
            let rel = relative_path(root, &file);
            let mut search = 0;
            while let Some(rel_idx) = content[search..].find("export const ") {
                let idx = search + rel_idx;
                search = idx + "export const ".len();
                let after = &content[search..];
                let var: String = after
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if var.is_empty() {
                    continue;
                }
                let rest = after[var.len()..].trim_start();
                if !rest.starts_with("= pgTable") && !rest.starts_with("=pgTable") {
                    continue;
                }
                let brace = match content[idx..].find('{') {
                    Some(b) => idx + b,
                    None => continue,
                };
                let props = extract_brace_body(&content, brace)
                    .map(|body| {
                        body.lines()
                            .filter_map(|l| {
                                let t = l.trim();
                                let ident: String = t
                                    .chars()
                                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                                    .collect();
                                if !ident.is_empty()
                                    && t[ident.len()..].trim_start().starts_with(':')
                                {
                                    Some(ident)
                                } else {
                                    None
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                entities.insert(
                    to_pascal_case(&var),
                    EntityInfo {
                        file: rel.clone(),
                        decorators: vec!["pgTable".to_string()],
                        properties: props,
                        ..EntityInfo::default()
                    },
                );
            }
        }
    }

    /// Prisma: `model Name { … }` declarations in `prisma/schema.prisma`.
    fn scan_prisma(root: &Path, entities: &mut BTreeMap<String, EntityInfo>) {
        let schema = root.join("prisma/schema.prisma");
        let Some(content) = read_file_safe(&schema) else {
            return;
        };
        let rel = relative_path(root, &schema);
        let mut search = 0;
        while let Some(rel_idx) = content[search..].find("model ") {
            let idx = search + rel_idx;
            // Must be at line start (ignore leading whitespace).
            let line_start = content[..idx].rfind('\n').map_or(0, |n| n + 1);
            if content[line_start..idx].trim().is_empty() {
                let after = &content[idx + "model ".len()..];
                let name: String = after
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    if let Some(brace) = content[idx..].find('{') {
                        let props = extract_brace_body(&content, idx + brace)
                            .map(|body| {
                                body.lines()
                                    .filter_map(|l| {
                                        let f: String = l
                                            .trim()
                                            .chars()
                                            .take_while(|c| {
                                                c.is_ascii_alphanumeric() || *c == '_'
                                            })
                                            .collect();
                                        (!f.is_empty() && !f.starts_with('@')).then_some(f)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        entities.insert(
                            name,
                            EntityInfo {
                                file: rel.clone(),
                                decorators: vec!["prisma-model".to_string()],
                                properties: props,
                                ..EntityInfo::default()
                            },
                        );
                    }
                }
            }
            search = idx + "model ".len();
        }
    }

    /// TypeORM: classes preceded by an `@Entity(...)` decorator.
    fn scan_typeorm(root: &Path, entities: &mut BTreeMap<String, EntityInfo>) {
        for file in collect_files(root, ".ts", &[]) {
            let Some(content) = read_file_safe(&file) else {
                continue;
            };
            if !content.contains("@Entity") {
                continue;
            }
            let rel = relative_path(root, &file);
            let mut search = 0;
            while let Some(rel_idx) = content[search..].find("@Entity") {
                let idx = search + rel_idx;
                search = idx + "@Entity".len();
                if let Some(class_off) = content[idx..].find("class ") {
                    let after = &content[idx + class_off + "class ".len()..];
                    let name: String = after
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        entities.entry(name).or_insert_with(|| EntityInfo {
                            file: rel.clone(),
                            decorators: vec!["@Entity".to_string()],
                            ..EntityInfo::default()
                        });
                    }
                }
            }
        }
    }
}

impl Scanner for TypeScriptScanner {
    fn detect(&self, root: &Path) -> bool {
        root.join("package.json").exists() || root.join("tsconfig.json").exists()
    }

    fn detect_architecture(&self, root: &Path) -> String {
        let src = root.join("src");
        let base = if src.exists() { src } else { root.to_path_buf() };
        let dir_count = std::fs::read_dir(&base)
            .map(|d| d.flatten().filter(|e| e.path().is_dir()).count())
            .unwrap_or(0);
        if dir_count <= 2 {
            "minimal".to_string()
        } else {
            "layered".to_string()
        }
    }

    fn scan_entities(&self, root: &Path) -> BTreeMap<String, EntityInfo> {
        let mut entities = BTreeMap::new();
        let fw = Self::detect_frameworks(root);
        if fw.drizzle {
            Self::scan_drizzle(root, &mut entities);
        }
        if fw.prisma {
            Self::scan_prisma(root, &mut entities);
        }
        if fw.typeorm {
            Self::scan_typeorm(root, &mut entities);
        }
        entities
    }

    fn scan_enums(&self, root: &Path) -> BTreeMap<String, EnumInfo> {
        let mut enums = BTreeMap::new();
        for file in collect_files(root, ".ts", &[]) {
            let Some(content) = read_file_safe(&file) else {
                continue;
            };
            if !content.contains("enum ") {
                continue;
            }
            let rel = relative_path(root, &file);
            let mut search = 0;
            while let Some(rel_idx) = content[search..].find("enum ") {
                let idx = search + rel_idx;
                search = idx + "enum ".len();
                // Require an `export` (optionally `export const`) before `enum`.
                let before = content[..idx].trim_end();
                if !before.ends_with("export") && !before.ends_with("export const") {
                    continue;
                }
                let after = &content[idx + "enum ".len()..];
                let name: String = after
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if name.is_empty() {
                    continue;
                }
                let Some(brace) = content[idx..].find('{') else {
                    continue;
                };
                let Some(body) = extract_brace_body(&content, idx + brace) else {
                    continue;
                };
                let values: Vec<String> = body
                    .split([',', '\n'])
                    .filter_map(|raw| {
                        let member = raw.trim();
                        let name = member.split('=').next().unwrap_or("").trim();
                        (!name.is_empty()
                            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
                        .then(|| name.to_string())
                    })
                    .collect();
                let convention = detect_value_convention(&values);
                enums.insert(
                    name,
                    EnumInfo {
                        values,
                        file: rel.clone(),
                        decorators: Vec::new(),
                        value_convention: Some(convention),
                    },
                );
            }
        }
        enums
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detect_requires_manifest() {
        let dir = tempdir().unwrap();
        assert!(!TypeScriptScanner.detect(dir.path()));
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();
        assert!(TypeScriptScanner.detect(dir.path()));
    }

    #[test]
    fn scan_entities_extracts_drizzle_table() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies":{"drizzle-orm":"0.1"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("schema.ts"),
            "export const users = pgTable('users', {\n  id: serial(),\n  name: text(),\n});\n",
        )
        .unwrap();
        let entities = TypeScriptScanner.scan_entities(dir.path());
        let users = entities.get("Users").expect("Users entity");
        assert_eq!(users.decorators, vec!["pgTable".to_string()]);
        assert!(users.properties.contains(&"id".to_string()));
        assert!(users.properties.contains(&"name".to_string()));
    }

    #[test]
    fn scan_enums_extracts_ts_enum() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        std::fs::write(
            dir.path().join("status.ts"),
            "export enum Status {\n  Active = 'active',\n  Closed = 'closed',\n}\n",
        )
        .unwrap();
        let enums = TypeScriptScanner.scan_enums(dir.path());
        let status = enums.get("Status").expect("Status enum");
        assert_eq!(status.values, vec!["Active", "Closed"]);
    }
}
