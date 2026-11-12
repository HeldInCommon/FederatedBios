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
/// A second published test key, for cosignatures. **Never use it for anything real.**
const TEST_KEY_B: &str = "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f";

/// The folder holding the spec's test vectors, `docs/spec/vectors`.
fn vectors_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/spec/vectors")
}

/// The vectors this implementation defines: (file name, description, envelope, cosigning keys).
fn cases() -> Vec<(&'static str, &'static str, Envelope, Vec<&'static str>)> {
    // The story text the "Only while I'm here" vectors (07, 08) are about:
    // the same words as the story in 05.
    let story_text = "The 38 sighs at the stop.\nA dog answers from a balcony.";
    let card_story = hic_commons::story::new_story(
        "0123456789abcdef",
        "tega",
        "Night bus",
        story_text,
        "commons",
        "cc-by-nc",
        &["credit".into(), "open".into()],
    )
    .unwrap();
    let card = hic_commons::here::card_for(&card_story, "http://room.example:8081");
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
        (
            "04-profile.json",
            "A shared Profile: every field signed, the picture included.",
            Envelope::new(
                "Profile",
                &a,
                1_700_000_200,
                None,
                json!({
                    "handle": "tega",
                    "display_name": "Tega Brain",
                    "visibility": "public",
                    "license": "cc-by",
                    "signals": ["credit"],
                    "profile": {
                        "name": "Tega Brain", "pronouns": "she/her", "role": "Artist",
                        "bio": "Works with environmental data.",
                        "places": [{ "name": "Studio", "city": "Sydney" }],
                        "avatar": "data:image/png;base64,AAAA",
                    },
                    "box_key": "cd".repeat(32),
                    "origin": "https://tega.example",
                    "origin_label": "Tega's node",
                    "version_cid": "1f4e0af70ee2d0a728728e0f045a139cbf7cfd34df02a0ea71372ad93b40f659",
                }),
            ),
            vec![],
        ),
        (
            "05-story.json",
            "A Story: a short piece of writing shared to a commons, with its terms.",
            Envelope::new(
                "Story",
                &a,
                1_700_000_300,
                None,
                json!({
                    "id": "0123456789abcdef",
                    "handle": "tega",
                    "title": "Night bus",
                    "text": "The 38 sighs at the stop.\nA dog answers from a balcony.",
                    "visibility": "commons",
                    "license": "cc-by-nc",
                    "signals": ["credit", "open"],
                }),
            ),
            vec![],
        ),
        (
            "06-story-withdraw.json",
            "A StoryWithdraw: removes one story from one commons (named by host).",
            Envelope::new(
                "StoryWithdraw",
                &a,
                1_700_000_400,
                None,
                json!({ "story": "0123456789abcdef", "commons": "room.example:8081" }),
            ),
            vec![],
        ),
        (
            "07-story-card.json",
            "A StoryCard (\"Only while I'm here\", decision record 0021): the story in 05 without its text, \
             carrying the SHA-256 of that text, addressed to one commons.",
            Envelope::new("StoryCard", &a, 1_700_000_500, None, serde_json::to_value(&card).unwrap()),
            vec![],
        ),
        (
            "08-story-relay.json",
            "A StoryRelay: a node's poll to the commons in 07, answering one reader's request with the story's \
             text, which must match the card's text_sha256.",
            Envelope::new(
                "StoryRelay",
                &a,
                1_700_000_600,
                None,
                json!({
                    "commons": "room.example:8081",
                    "seq": 1_700_000_600_000u64,
                    "answers": [{
                        "request": "00112233445566778899aabbccddeeff",
                        "story": "0123456789abcdef",
                        "text": story_text,
                    }],
                }),
            ),
            vec![],
        ),
    ]
}

/// The "Only while I'm here" vectors are what this implementation accepts,
/// and the card's hash is the hash of the story's text in 05.
#[test]
fn only_while_here_vectors_are_accepted() {
    let read = |name: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(vectors_dir().join(name)).unwrap()).unwrap()
    };
    let hosts = ["room.example:8081".to_string()];
    let card_msg =
        json!({ "type": "StoryCardShare", "schema": "hic/v1", "card": read("07-story-card.json")["signed"] });
    let (card, _) = hic_commons::here::verify_card(&card_msg, &hosts).unwrap();
    let story_text = read("05-story.json")["envelope"]["body"]["text"].as_str().unwrap().to_string();
    assert!(hic_commons::here::text_matches(&card, &story_text));
    let relay_msg = json!({ "type": "StoryRelay", "schema": "hic/v1", "relay": read("08-story-relay.json")["signed"] });
    let relay = hic_commons::here::verify_relay(&relay_msg, &hosts, 1_700_000_600).unwrap();
    assert!(hic_commons::here::text_matches(&card, &relay.answers[0].text));
}

/// The story vectors are what this implementation's story code accepts.
#[test]
fn story_vectors_are_accepted() {
    let read = |name: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(vectors_dir().join(name)).unwrap()).unwrap()
    };
    let share = json!({ "type": "StoryShare", "schema": "hic/v1", "story": read("05-story.json")["signed"] });
    let (story, _) = hic_commons::story::verify_share(&share).unwrap();
    assert_eq!(story.title, "Night bus");
    let withdraw =
        json!({ "type": "StoryWithdraw", "schema": "hic/v1", "withdraw": read("06-story-withdraw.json")["signed"] });
    let w = hic_commons::story::verify_withdraw(&withdraw, &["room.example:8081".to_string()]).unwrap();
    assert_eq!(w.id, "0123456789abcdef");
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
