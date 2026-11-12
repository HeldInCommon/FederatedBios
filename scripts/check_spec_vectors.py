#!/usr/bin/env python3
"""Independent check of the hic/v1 test vectors (docs/spec/vectors/).

Recomputes, without any Held in Common code, the canonical JSON (RFC 8785,
integers only), the cid (SHA-256) and the signing input for each vector, and
compares them with the published values. A second implementation agreeing
with the first is the point of a written specification.

It also checks the "Only while I'm here" pair (decision record 0021): the
card in 07 carries the SHA-256 of the story text in 05, and the answer in the
poll in 08 is text that matches it. That hash is what lets a reader catch a
commons that altered the words.

Signatures aren't checked here because Python's standard library has no
Ed25519; `cargo test --test spec_vectors` checks those.

Usage: python3 scripts/check_spec_vectors.py
"""
import hashlib
import json
import pathlib
import sys

VECTORS = pathlib.Path(__file__).resolve().parent.parent / "docs" / "spec" / "vectors"
MAX_SAFE_INT = 2**53 - 1


def canonical(value):
    """RFC 8785 canonical JSON for strings, integers, booleans, null, arrays, objects."""
    if value is None:
        return "null"
    if value is True:
        return "true"
    if value is False:
        return "false"
    if isinstance(value, int):
        if abs(value) > MAX_SAFE_INT:
            raise ValueError("integer outside the safe range")
        return str(value)
    if isinstance(value, str):
        # Python's escaping matches RFC 8785 when non-ASCII is left raw:
        # \" \\ \b \f \n \r \t, other control characters as lowercase \u00xx.
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, list):
        return "[" + ",".join(canonical(v) for v in value) + "]"
    if isinstance(value, dict):
        keys = sorted(value, key=lambda k: k.encode("utf-16-be"))  # UTF-16 code-unit order
        return "{" + ",".join(json.dumps(k, ensure_ascii=False) + ":" + canonical(value[k]) for k in keys) + "}"
    raise ValueError(f"not allowed in signed objects: {value!r}")


def main():
    failures = 0
    files = sorted(VECTORS.glob("*.json"))
    for path in files:
        v = json.loads(path.read_text(encoding="utf-8"))
        text = canonical(v["envelope"])
        cid = hashlib.sha256(text.encode("utf-8")).hexdigest()
        signing_input = f"hic/v1/{v['envelope']['type']}\n{cid}"
        checks = [
            ("canonical", text == v["canonical"]),
            ("cid", cid == v["cid"]),
            ("signing input", signing_input == v["signing_input"]),
        ]
        bad = [name for name, ok in checks if not ok]
        print(("FAIL " if bad else "ok   ") + path.name + (f" ({', '.join(bad)})" if bad else ""))
        failures += bool(bad)
    print(f"{len(files) - failures}/{len(files)} vectors match")
    text_failures = check_card_hashes()
    return 1 if failures or text_failures or not files else 0


def sha256_hex(text):
    """Lowercase hex SHA-256 of a string's UTF-8 bytes (spec section 5.5)."""
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def check_card_hashes():
    """The StoryCard's text_sha256 is the hash of the Story's text (05), and
    every StoryRelay answer (08) for that story matches it. Returns the
    number of failures."""
    def body(name):
        return json.loads((VECTORS / name).read_text(encoding="utf-8"))["envelope"]["body"]

    try:
        story, card, relay = body("05-story.json"), body("07-story-card.json"), body("08-story-relay.json")
    except FileNotFoundError as missing:
        print(f"FAIL card hashes (missing {missing.filename})")
        return 1
    checks = [("card hash is the story text's", card["text_sha256"] == sha256_hex(story["text"]))]
    for answer in relay["answers"]:
        if answer["story"] == card["id"]:
            checks.append(("answer matches the card", sha256_hex(answer["text"]) == card["text_sha256"]))
    # And an altered text would not match (the point of the hash).
    checks.append(("altered text is caught", sha256_hex(story["text"] + " ") != card["text_sha256"]))
    bad = [name for name, ok in checks if not ok]
    print(("FAIL " if bad else "ok   ") + "card hashes" + (f" ({', '.join(bad)})" if bad else ""))
    return len(bad)


if __name__ == "__main__":
    sys.exit(main())
