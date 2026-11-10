#!/usr/bin/env python3
"""Independent check of the hic/v1 test vectors (docs/spec/vectors/).

Recomputes, without any Held in Common code, the canonical JSON (RFC 8785,
integers only), the cid (SHA-256) and the signing input for each vector, and
compares them with the published values. A second implementation agreeing
with the first is the point of a written specification.

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
    return 1 if failures or not files else 0


if __name__ == "__main__":
    sys.exit(main())
