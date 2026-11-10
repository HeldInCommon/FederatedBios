//! Checks the published test vectors in `docs/spec/vectors/` against this
//! implementation: the canonical bytes, the `cid`, the signing input and the
//! signatures must all match exactly.
//!
//! To regenerate the vectors after an *intentional* format change (which
//! needs a new schema version, see `docs/spec/README.md`):
//!
//! ```text
//! HIC_WRITE_VECTORS=1 cargo test --test spec_vectors
//! ```

use std::path::PathBuf;

use hic_commons::envelope::{self, Envelope};
use hic_commons::keys;
use serde_json::{json, Value};

/// Published test keys. **Never use these for anything real.**
const TEST_KEY_A: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const TEST_KEY_B: &str = "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f";

fn vectors_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/spec/vectors")
}

/// The vectors this implementation defines: (file name, description, envelope, cosigning keys).
fn cases() -> Vec<(&'static str, &'static str, Envelope, Vec<&'static str>)> {
    let a = keys::public_from_private(TEST_KEY_A).unwrap();
    let b = keys::public_from_private(TEST_KEY_B).unwrap();
    vec![
        (
            "01-basic.json",
            "The smallest useful object: one string in the body, no previous object.",
            Envelope::new("Note", &a, 1_700_000_000, None, json!({ "text": "hello" })),
            vec![],
        ),
        (
            "02-ordering-and-escaping.json",
            "Key sorting (including a character outside the Basic Multilingual Plane, which sorts by UTF-16), \
             string escaping, negative integers, booleans and null.",
            Envelope::new(
                "Note",
                &a,
                1_700_000_001,
                Some("0".repeat(64)),
                json!({
                    "z": "line\nbreak, \"quoted\", tab\t, control\u{1}, é, 😀, /slash",
                    "a": [1, -2, true, null],
                    "\u{FB33}": "after the emoji key",
                    "\u{1F600}": "before U+FB33 in UTF-16 order",
                }),
            ),
            vec![],
        ),
        (
            "03-key-rotation.json",
            "A KeyRotation signed by the old key (A) and cosigned by the new key (B).",
            Envelope::new(
                "KeyRotation",
                &a,
                1_700_000_100,
                None,
                json!({ "handle": "tega", "new_key": b, "new_box_key": "ab".repeat(32) }),
            ),
            vec![TEST_KEY_B],
        ),
    ]
}

/// Build the vector file contents for one case.
fn vector(name: &str, description: &str, env: &Envelope, cosigners: &[&str]) -> Value {
    let canonical = String::from_utf8(envelope::canonical_bytes(env).unwrap()).unwrap();
    let mut signed = envelope::sign(env.clone(), TEST_KEY_A).unwrap();
    for k in cosigners {
        signed = envelope::cosign(signed, k).unwrap();
    }
    json!({
        "name": name,
        "description": description,
        "test_private_keys": { "A": TEST_KEY_A, "B": TEST_KEY_B },
        "envelope": env,
        "canonical": canonical,
        "cid": signed.cid,
        "signing_input": envelope::signing_input(&env.kind, &signed.cid),
        "signed": signed,
    })
}

#[test]
fn published_vectors_match_this_implementation() {
    let write = std::env::var("HIC_WRITE_VECTORS").is_ok();
    for (name, description, env, cosigners) in cases() {
        let expected = vector(name, description, &env, &cosigners);
        let path = vectors_dir().join(name);
        if write {
            std::fs::create_dir_all(vectors_dir()).unwrap();
            std::fs::write(&path, serde_json::to_string_pretty(&expected).unwrap() + "\n").unwrap();
            continue;
        }
        let published: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(published, expected, "{name} differs from this implementation");
        // And the published signed object verifies on its own.
        let signed: envelope::Signed = serde_json::from_value(published["signed"].clone()).unwrap();
        envelope::verify(&signed).unwrap();
    }
}
