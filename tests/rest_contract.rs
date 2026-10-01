//! REST contract: the published OpenAPI document against the routes the bridge
//! actually declares.
//!
//! The spec is a file baked into the binary (`include_str!("../openapi.json")` in
//! `get_openapi`), so a route added without a spec update, or a spec entry left
//! behind after a rename, is served as fact and never fails anything. This
//! compares the two sets in both directions, because either error is a lie
//! published to clients.
//!
//! Read from the sources rather than from a live socket on purpose: it needs no
//! index, no port, no client dependency, and it runs in CI on every push. What
//! it deliberately does not check is behaviour -- status codes and auth need the
//! server up, and the audit asks for those separately.

use std::collections::BTreeSet;

use serde_json::Value;

const SPEC: &str = include_str!("../openapi.json");
const BRIDGE: &str = include_str!("../src/bridge.rs");

/// Every path the bridge mounts under `/v1`, read out of the `.route(...)` calls
/// that build `v1_routes`. Kept as source text rather than a duplicated list on
/// purpose: a list written out here would drift silently, which is the exact
/// failure this test exists to catch.
fn declared_v1_paths() -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let mut in_v1 = false;
    for line in BRIDGE.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("let v1_routes") {
            in_v1 = true;
            continue;
        }
        // The block ends where the outer router starts. Everything after it is
        // the unversioned backward-compatibility aliases, which are mounted but
        // deliberately not part of the documented contract.
        if in_v1 && trimmed.starts_with("let app") {
            break;
        }
        if !in_v1 {
            continue;
        }
        if let Some(rest) = trimmed.split(".route(\"").nth(1) {
            if let Some(path) = rest.split('"').next() {
                set.insert(format!("/v1{path}"));
            }
        }
    }
    set
}

fn spec_paths() -> BTreeSet<String> {
    let doc: Value = serde_json::from_str(SPEC).expect("openapi.json parses");
    assert!(
        doc["openapi"]
            .as_str()
            .unwrap_or_default()
            .starts_with("3."),
        "openapi.json should declare a 3.x document, found {:?}",
        doc["openapi"]
    );
    doc["paths"]
        .as_object()
        .expect("openapi.json has a paths object")
        .keys()
        .cloned()
        .collect()
}

#[test]
fn every_documented_path_is_a_real_route() {
    let declared = declared_v1_paths();
    assert!(
        !declared.is_empty(),
        "route extraction found nothing; the anchor in bridge.rs moved"
    );
    let documented = spec_paths();
    let mut missing = Vec::new();
    for path in &documented {
        // /openapi.json is mounted at the root, not under /v1.
        if path == "/openapi.json" {
            assert!(
                BRIDGE.contains(".route(\"/openapi.json\""),
                "the spec serves {path} but bridge.rs does not mount it"
            );
            continue;
        }
        if !declared.contains(path) {
            missing.push(path.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "openapi.json documents routes the bridge does not mount: {missing:?}"
    );
}

#[test]
fn every_v1_route_is_documented() {
    let declared = declared_v1_paths();
    let documented = spec_paths();
    let mut undocumented: Vec<_> = declared.difference(&documented).cloned().collect();
    undocumented.sort();
    assert!(
        undocumented.is_empty(),
        "the bridge mounts routes that openapi.json does not document: \
         {undocumented:?}"
    );
}
