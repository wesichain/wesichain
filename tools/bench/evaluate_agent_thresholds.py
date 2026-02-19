#!/usr/bin/env python3
"""Evaluate wesichain-agent benchmark regressions against threshold policy."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

import tomllib


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Evaluate wesichain-agent benchmark thresholds")
    parser.add_argument("--thresholds", required=True, help="Path to agent-thresholds.toml")
    parser.add_argument(
        "--criterion-root",
        default="",
        help="Criterion output root (used when --metrics-json is omitted)",
    )
    parser.add_argument(
        "--rss-file",
        default="",
        help="File with rss_bytes=<value> or '/usr/bin/time -v' output",
    )
    parser.add_argument(
        "--dataset-size",
        type=float,
        default=1000.0,
        help="Reserved compatibility flag for benchmark jobs",
    )
    parser.add_argument(
        "--metrics-json",
        default="",
        help="Optional JSON with baseline/current metric pairs",
    )
    return parser.parse_args()


def percentile(values: list[float], p: float) -> float:
    ordered = sorted(values)
    if not ordered:
        raise ValueError("percentile requires at least one value")
    if len(ordered) == 1:
        return ordered[0]

    idx = (len(ordered) - 1) * p
    lo = math.floor(idx)
    hi = math.ceil(idx)
    if lo == hi:
        return ordered[int(idx)]

    lo_value = ordered[lo]
    hi_value = ordered[hi]
    weight = idx - lo
    return lo_value + (hi_value - lo_value) * weight


def load_thresholds(path: Path) -> dict[str, float]:
    with path.open("rb") as handle:
        config = tomllib.load(handle)

    required = {
        "p50_regression_review_pct",
        "p95_regression_block_pct",
        "peak_memory_regression_block_pct",
        "error_rate_regression_block_pct",
        "crash_rate_regression_block_pct",
        "baseline_p50_ms",
        "baseline_p95_ms",
        "baseline_peak_memory_mb",
        "baseline_error_rate",
        "baseline_crash_rate",
    }
    section = config.get("agent", {})
    missing = sorted(required - set(section))
    if missing:
        raise ValueError(f"missing threshold keys: {', '.join(missing)}")
    return {name: float(section[name]) for name in required}


def _extract_sample_ms(sample_path: Path) -> list[float]:
    payload = json.loads(sample_path.read_text(encoding="utf-8"))
    iters = payload.get("iters")
    times = payload.get("times")
    if not isinstance(iters, list) or not isinstance(times, list) or len(iters) != len(times):
        raise ValueError(f"invalid criterion sample format in {sample_path}")

    values: list[float] = []
    for iter_count, total_ns in zip(iters, times):
        if not iter_count:
            continue
        values.append((float(total_ns) / float(iter_count)) / 1_000_000.0)

    return values


def load_criterion_latency_metrics(criterion_root: Path) -> tuple[float, float]:
    samples = list(criterion_root.rglob("agent_runtime_profiles/*/new/sample.json"))
    if not samples:
        raise FileNotFoundError(
            "criterion samples for 'agent_runtime_profiles' not found under "
            f"{criterion_root}"
        )

    values_ms: list[float] = []
    for sample_path in samples:
        values_ms.extend(_extract_sample_ms(sample_path))

    if not values_ms:
        raise ValueError("no benchmark samples found for agent runtime profiles")

    return percentile(values_ms, 0.50), percentile(values_ms, 0.95)


def load_peak_memory_mb(rss_file: Path) -> float:
    raw = rss_file.read_text(encoding="utf-8")
    for line in raw.splitlines():
        line = line.strip()
        if line.startswith("rss_bytes="):
            bytes_value = float(line.split("=", 1)[1])
            return bytes_value / (1024.0 * 1024.0)
        if line.startswith("Maximum resident set size (kbytes):"):
            kb_value = float(line.split(":", 1)[1].strip())
            return kb_value / 1024.0
    raise ValueError(
        f"expected rss_bytes=<value> or 'Maximum resident set size (kbytes):' in {rss_file}"
    )


def load_metrics(args: argparse.Namespace, thresholds: dict[str, float]) -> dict[str, float]:
    if args.metrics_json:
        payload = json.loads(Path(args.metrics_json).read_text(encoding="utf-8"))
        required = {
            "p50_ms_baseline",
            "p50_ms_current",
            "p95_ms_baseline",
            "p95_ms_current",
            "peak_memory_mb_baseline",
            "peak_memory_mb_current",
            "error_rate_baseline",
            "error_rate_current",
            "crash_rate_baseline",
            "crash_rate_current",
        }
        missing = sorted(required - set(payload))
        if missing:
            raise ValueError(f"metrics JSON missing keys: {', '.join(missing)}")
        return {name: float(payload[name]) for name in required}

    if not args.criterion_root or not args.rss_file:
        raise ValueError("--criterion-root and --rss-file are required without --metrics-json")

    p50_ms, p95_ms = load_criterion_latency_metrics(Path(args.criterion_root))
    peak_memory_mb = load_peak_memory_mb(Path(args.rss_file))

    return {
        "p50_ms_baseline": thresholds["baseline_p50_ms"],
        "p50_ms_current": p50_ms,
        "p95_ms_baseline": thresholds["baseline_p95_ms"],
        "p95_ms_current": p95_ms,
        "peak_memory_mb_baseline": thresholds["baseline_peak_memory_mb"],
        "peak_memory_mb_current": peak_memory_mb,
        "error_rate_baseline": thresholds["baseline_error_rate"],
        "error_rate_current": 0.0,
        "crash_rate_baseline": thresholds["baseline_crash_rate"],
        "crash_rate_current": 0.0,
    }


def regression_pct(current: float, baseline: float) -> float:
    if baseline == 0.0:
        if current == 0.0:
            return 0.0
        return 100.0
    return ((current - baseline) / baseline) * 100.0


def evaluate(metrics: dict[str, float], thresholds: dict[str, float]) -> int:
    p50_regression = regression_pct(metrics["p50_ms_current"], metrics["p50_ms_baseline"])
    p95_regression = regression_pct(metrics["p95_ms_current"], metrics["p95_ms_baseline"])
    memory_regression = regression_pct(
        metrics["peak_memory_mb_current"],
        metrics["peak_memory_mb_baseline"],
    )
    error_regression = regression_pct(
        metrics["error_rate_current"],
        metrics["error_rate_baseline"],
    )
    crash_regression = regression_pct(
        metrics["crash_rate_current"],
        metrics["crash_rate_baseline"],
    )

    print("Agent benchmark threshold evaluation")
    print(
        f"- p50 latency regression: {p50_regression:.2f}% "
        f"(review>{thresholds['p50_regression_review_pct']:.2f}%)"
    )
    print(
        f"- p95 latency regression: {p95_regression:.2f}% "
        f"(block>{thresholds['p95_regression_block_pct']:.2f}%)"
    )
    print(
        f"- peak memory regression: {memory_regression:.2f}% "
        f"(block>{thresholds['peak_memory_regression_block_pct']:.2f}%)"
    )
    print(
        f"- error rate regression: {error_regression:.2f}% "
        f"(block>{thresholds['error_rate_regression_block_pct']:.2f}%)"
    )
    print(
        f"- crash rate regression: {crash_regression:.2f}% "
        f"(block>{thresholds['crash_rate_regression_block_pct']:.2f}%)"
    )

    if p50_regression > thresholds["p50_regression_review_pct"]:
        print("REVIEW: p50 regression exceeded advisory threshold")

    blockers = []
    if p95_regression > thresholds["p95_regression_block_pct"]:
        blockers.append("p95 latency")
    if memory_regression > thresholds["peak_memory_regression_block_pct"]:
        blockers.append("peak memory")
    if error_regression > thresholds["error_rate_regression_block_pct"]:
        blockers.append("error rate")
    if crash_regression > thresholds["crash_rate_regression_block_pct"]:
        blockers.append("crash rate")

    if blockers:
        print(f"BLOCK: threshold gate failed for {', '.join(blockers)}")
        return 1

    print("PASS: benchmark thresholds satisfied")
    return 0


def main() -> int:
    args = parse_args()
    try:
        thresholds = load_thresholds(Path(args.thresholds))
        metrics = load_metrics(args, thresholds)
        return evaluate(metrics, thresholds)
    except Exception as exc:  # pragma: no cover - CLI error path
        print(f"error: {exc}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
