#!/usr/bin/env python3
"""Guard against stale README version and claim drift."""

from __future__ import annotations

from pathlib import Path


def main() -> int:
    readme = Path("README.md")
    if not readme.exists():
        print("error: README.md not found")
        return 2

    text = readme.read_text(encoding="utf-8")

    failures: list[str] = []

    banned_tokens = [
        "0.1.0",
        '"0.2.0"',
    ]
    for token in banned_tokens:
        if token in text:
            failures.append(f"README contains banned token: {token}")

    required_tokens = [
        "v0.2.1",
        "ReActGraphBuilder",
        "wesichain-memory",
    ]
    for token in required_tokens:
        if token not in text:
            failures.append(f"README missing required token: {token}")

    if failures:
        print("docs claim guard failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1

    print("docs claim guard passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
