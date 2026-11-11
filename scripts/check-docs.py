#!/usr/bin/env python3
"""Check that the Rust code is documented (house style §8, "Comment all code well").

Looks at every `.rs` file in commons/src, commons/tests and src-tauri/src and
reports:

  - a file that doesn't start with a `//!` comment saying what it is for
  - a function, struct, enum, trait, type alias, const or static with no `///`
    comment directly above it (attribute lines such as `#[derive(...)]` may sit
    in between). Test functions (`#[test]`, `#[tokio::test]`) are skipped:
    their names are sentences that state the rule they test.

Struct fields and enum variants aren't checked: whether a field needs a comment
is a judgement call ("explain why, not what"), which a script can't make.

This is a line-based scan, not a Rust parser, so it is deliberately simple. It
only uses Python's standard library.

Usage: python3 scripts/check-docs.py        exit 0 when clean, 1 with a list
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
FOLDERS = ["commons/src", "commons/tests", "src-tauri/src"]

# `pub`, `pub(crate)`, `pub(super)` … then `const`/`async`/`unsafe` before `fn`.
VISIBILITY = r"(pub(\([^)]*\))?\s+)?"
FUNCTION = re.compile(VISIBILITY + r"((const|async|unsafe)\s+)*fn\s+\w")
ITEM = re.compile(VISIBILITY + r"(struct|enum|trait|type|const|static)\s+\w")
TEST_ATTRIBUTE = re.compile(r"#\[(\w+::)?test\b")


def problems_in(path: pathlib.Path) -> list[str]:
    """Every undocumented item in one file, as `path:line: text` strings."""
    lines = path.read_text(encoding="utf-8").split("\n")
    name = path.relative_to(ROOT)
    found = []

    first = next((line.strip() for line in lines if line.strip()), "")
    if not first.startswith("//!"):
        found.append(f"{name}:1: no //! comment saying what this file is for")

    for number, line in enumerate(lines, start=1):
        text = line.strip()
        if not (FUNCTION.match(text) or ITEM.match(text)):
            continue
        # Step up past attribute lines to the line that should be the comment.
        above = number - 2
        attributes = []
        while above >= 0 and lines[above].strip().startswith("#["):
            attributes.append(lines[above].strip())
            above -= 1
        if any(TEST_ATTRIBUTE.match(a) for a in attributes):
            continue
        if above < 0 or not lines[above].strip().startswith("///"):
            found.append(f"{name}:{number}: {text[:80]}")
    return found


def main() -> int:
    """Scan every folder, print what's missing, and return the exit code."""
    found = []
    for folder in FOLDERS:
        for path in sorted((ROOT / folder).rglob("*.rs")):
            found.extend(problems_in(path))
    if found:
        print("Missing doc comments (house style §8, \"Comment all code well\"):")
        print("\n".join(f"  {p}" for p in found))
        print(f"\n{len(found)} to fix: add a /// comment that says what it is for, and why.")
        return 1
    print("Every file, function and type has a doc comment.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
