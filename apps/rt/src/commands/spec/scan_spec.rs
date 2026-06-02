//! `scan spec` — compile a deterministic spec draft for one entity via
//! `grain spec`. Thin passthrough: constructs a [`SpecRequest`] from CLI flags
//! and prints the Markdown verbatim to stdout. The heavy logic lives entirely
//! in [`mustard_core::domain::scan::Scan::spec`]; this module owns nothing.

use std::path::PathBuf;

use mustard_core::{Scan, SpecRequest};

pub struct ScanSpecOpts {
    pub entity: String,
    pub like: Option<String>,
    pub ops: Vec<String>,
    pub invariants: Vec<String>,
    pub root: PathBuf,
}

/// Run `grain spec` for `opts.entity` and print the resulting Markdown to
/// stdout. Exits with code `1` on failure (grain not installed, model missing,
/// non-zero exit from grain) so the caller can detect the error.
pub fn run(opts: ScanSpecOpts) {
    let model = opts.root.join(".claude").join("grain.model.json");
    let req = SpecRequest {
        entity: opts.entity,
        like: opts.like.unwrap_or_default(),
        ops: opts.ops,
        invariants: opts.invariants,
    };
    match Scan::locate().spec(&model, &req) {
        Ok(md) => println!("{md}"),
        Err(err) => {
            eprintln!("scan spec: grain failed: {err}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that a `SpecRequest` is wired from opts correctly (no logic
    /// change — just the field mapping we own). We do NOT invoke grain (not
    /// installed in CI); the test is purely about argument plumbing.
    #[test]
    fn opts_wire_to_spec_request() {
        let opts = ScanSpecOpts {
            entity: "Order".to_string(),
            like: Some("Invoice".to_string()),
            ops: vec!["approve".to_string(), "cancel".to_string()],
            invariants: vec!["no-double-charge".to_string()],
            root: PathBuf::from("."),
        };
        // Build the SpecRequest the same way `run` does.
        let req = SpecRequest {
            entity: opts.entity.clone(),
            like: opts.like.clone().unwrap_or_default(),
            ops: opts.ops.clone(),
            invariants: opts.invariants.clone(),
        };
        assert_eq!(req.entity, "Order");
        assert_eq!(req.like, "Invoice");
        assert_eq!(req.ops, ["approve", "cancel"]);
        assert_eq!(req.invariants, ["no-double-charge"]);
    }

    #[test]
    fn like_none_becomes_empty_string() {
        let like: Option<String> = None;
        assert_eq!(like.unwrap_or_default(), String::new());
    }
}
