#!/usr/bin/env python3
"""Check release-readiness evidence for a migration slice."""

from __future__ import annotations

import argparse
from pathlib import Path


SLICE_CONFIG: dict[str, dict[str, str]] = {
    "qdrant": {
        "scoreboard_done_row": "| Qdrant | DONE |",
        "benchmark_artifact": "docs/benchmarks/data/qdrant-2026-02-16.json",
        "migration_guide": "docs/migration/langchain-to-wesichain-qdrant.md",
    },
    "weaviate": {
        "scoreboard_done_row": "| Weaviate | DONE |",
        "benchmark_artifact": "docs/benchmarks/data/weaviate-2026-02-16.json",
        "migration_guide": "docs/migration/langchain-to-wesichain-weaviate.md",
    },
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate migration slice readiness evidence")
    parser.add_argument("--slice", required=True, help="Slice key, e.g. qdrant")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    slice_key = args.slice.strip().lower()
    if slice_key not in SLICE_CONFIG:
        expected = ", ".join(sorted(SLICE_CONFIG.keys()))
        print(f"error: unsupported slice '{slice_key}', expected one of: {expected}")
        return 2

    config = SLICE_CONFIG[slice_key]
    root = Path(__file__).resolve().parents[2]
    scoreboard = root / "docs/migration/scoreboard.md"
    benchmark_artifact = root / config["benchmark_artifact"]
    migration_guide = root / config["migration_guide"]

    failures: list[str] = []

    if not scoreboard.exists():
        failures.append(f"missing scoreboard: {scoreboard}")
    else:
        scoreboard_text = scoreboard.read_text(encoding="utf-8")
        if config["scoreboard_done_row"] not in scoreboard_text:
            failures.append(f"scoreboard row for '{slice_key}' is not DONE yet")
        if "<nightly-build-url>" in scoreboard_text:
            failures.append("nightly evidence placeholder has not been replaced")
        if "<issue-url>" in scoreboard_text:
            failures.append("migration-unblocked issue placeholder has not been replaced")

    if not benchmark_artifact.exists():
        failures.append(f"missing benchmark artifact: {benchmark_artifact}")

    if not migration_guide.exists():
        failures.append(f"missing migration guide: {migration_guide}")

    if failures:
        print("Release readiness: FAIL")
        for failure in failures:
            print(f"- {failure}")
        return 1

    print("Release readiness: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
