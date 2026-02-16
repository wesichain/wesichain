#!/usr/bin/env python3
"""Evaluate nightly benchmark metrics against locked thresholds."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import math
from pathlib import Path
from typing import Any

import tomllib


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Evaluate qdrant benchmark thresholds")
    parser.add_argument("--thresholds", required=True, help="Path to thresholds.toml")
    parser.add_argument(
        "--criterion-root",
        required=True,
        help="Root directory containing criterion benchmark output",
    )
    parser.add_argument(
        "--rss-file",
        required=True,
        help="File containing rss_bytes=<value> or '/usr/bin/time -v' output",
    )
    parser.add_argument(
        "--waivers",
        default="",
        help="Optional path to WAIVERS.yml used to suppress failing metric checks",
    )
    parser.add_argument(
        "--dataset-size",
        type=float,
        default=1000.0,
        help="Documents processed per run for throughput estimate",
    )
    parser.add_argument(
        "--metrics-json",
        default="",
        help="Optional precomputed metrics JSON (overrides criterion parsing)",
    )
    return parser.parse_args()


def load_thresholds(path: Path) -> dict[str, float]:
    with path.open("rb") as f:
        config = tomllib.load(f)
    section = config.get("qdrant", {})
    required = {
        "query_p50",
        "query_p95",
        "query_p99",
        "index_throughput",
        "peak_memory",
        "error_rate",
    }
    missing = sorted(required - set(section))
    if missing:
        raise ValueError(f"missing threshold keys: {', '.join(missing)}")
    return {k: float(section[k]) for k in required}


def load_sample_times_ms(criterion_root: Path) -> list[float]:
    candidates = list(criterion_root.rglob("wesichain_payload/new/sample.json"))
    if not candidates:
        raise FileNotFoundError("criterion sample.json not found for wesichain_payload")

    sample_path = max(candidates, key=lambda candidate: (candidate.stat().st_mtime, str(candidate)))
    sample = json.loads(sample_path.read_text(encoding="utf-8"))
    iters = sample.get("iters")
    times = sample.get("times")
    if not isinstance(iters, list) or not isinstance(times, list) or len(iters) != len(times):
        raise ValueError(f"invalid sample format in {sample_path}")

    values_ms: list[float] = []
    for iter_count, total_ns in zip(iters, times):
        if not iter_count:
            continue
        per_iter_ns = float(total_ns) / float(iter_count)
        values_ms.append(per_iter_ns / 1_000_000.0)

    if not values_ms:
        raise ValueError(f"no benchmark values found in {sample_path}")
    return values_ms


def percentile(values: list[float], p: float) -> float:
    if not values:
        raise ValueError("values cannot be empty")
    ordered = sorted(values)
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


def load_peak_memory_gb(rss_file: Path) -> float:
    raw = rss_file.read_text(encoding="utf-8")
    for line in raw.splitlines():
        line = line.strip()
        if line.startswith("rss_bytes="):
            bytes_value = float(line.split("=", 1)[1])
            return bytes_value / (1024.0 * 1024.0 * 1024.0)
        if line.startswith("Maximum resident set size (kbytes):"):
            kb_value = float(line.split(":", 1)[1].strip())
            return kb_value * 1024.0 / (1024.0 * 1024.0 * 1024.0)
    raise ValueError(
        f"expected rss_bytes=<value> or 'Maximum resident set size (kbytes):' in {rss_file}"
    )


def _parse_scalar(value: str) -> str:
    text = value.strip()
    if not text:
        return ""
    if len(text) >= 2 and text[0] == text[-1] and text[0] in {"\"", "'"}:
        return text[1:-1]
    return text


def _parse_waivers_yaml(path: Path) -> list[dict[str, str]]:
    lines = path.read_text(encoding="utf-8").splitlines()
    waivers: list[dict[str, str]] = []
    in_waivers = False
    current: dict[str, str] | None = None

    for raw_line in lines:
        if not raw_line.strip() or raw_line.lstrip().startswith("#"):
            continue

        if not in_waivers:
            if raw_line.strip() == "waivers:":
                in_waivers = True
                continue
            if raw_line.strip() == "waivers: []":
                return []
            raise ValueError(f"invalid WAIVERS.yml structure in {path}: expected 'waivers:' root key")

        indent = len(raw_line) - len(raw_line.lstrip(" "))
        line = raw_line.strip()

        if line.startswith("- "):
            if current is not None:
                waivers.append(current)
            current = {}
            rest = line[2:].strip()
            if rest:
                if ":" not in rest:
                    raise ValueError(f"invalid waiver entry line in {path}: {raw_line}")
                key, value = rest.split(":", 1)
                current[key.strip()] = _parse_scalar(value)
            continue

        if current is None:
            raise ValueError(f"invalid WAIVERS.yml structure in {path}: field without list item")
        if indent < 2:
            raise ValueError(f"invalid WAIVERS.yml indentation in {path}: {raw_line}")
        if ":" not in line:
            raise ValueError(f"invalid waiver field in {path}: {raw_line}")

        key, value = line.split(":", 1)
        current[key.strip()] = _parse_scalar(value)

    if current is not None:
        waivers.append(current)

    return waivers


def load_active_waivers(
    path: Path | None,
    allowed_metrics: set[str],
) -> dict[str, dict[str, str]]:
    if path is None:
        return {}

    waivers = _parse_waivers_yaml(path)
    required_fields = {"owner", "reason", "expiry", "issue", "metric"}
    today = dt.date.today()
    active: dict[str, dict[str, str]] = {}

    for idx, waiver in enumerate(waivers, start=1):
        missing = sorted(field for field in required_fields if not waiver.get(field))
        if missing:
            raise ValueError(f"waiver #{idx} missing required fields: {', '.join(missing)}")

        metric = waiver["metric"]
        if metric not in allowed_metrics:
            allowed_sorted = ", ".join(sorted(allowed_metrics))
            raise ValueError(
                f"waiver #{idx} has unknown metric '{metric}', expected one of: {allowed_sorted}"
            )

        try:
            expiry = dt.date.fromisoformat(waiver["expiry"])
        except ValueError as exc:
            raise ValueError(
                f"waiver #{idx} has invalid expiry '{waiver['expiry']}', expected YYYY-MM-DD"
            ) from exc

        if expiry < today:
            raise ValueError(
                f"waiver #{idx} for metric '{metric}' expired on {expiry.isoformat()}"
            )

        active[metric] = waiver

    return active


def build_metrics(args: argparse.Namespace) -> dict[str, float]:
    if args.metrics_json:
        payload = json.loads(Path(args.metrics_json).read_text(encoding="utf-8"))
        return {k: float(v) for k, v in payload.items()}

    sample_values_ms = load_sample_times_ms(Path(args.criterion_root))
    p50 = percentile(sample_values_ms, 0.50)
    p95 = percentile(sample_values_ms, 0.95)
    p99 = percentile(sample_values_ms, 0.99)
    throughput = float(args.dataset_size) / (p50 / 1000.0) if p50 > 0 else 0.0
    peak_memory = load_peak_memory_gb(Path(args.rss_file))

    return {
        "query_p50": p50,
        "query_p95": p95,
        "query_p99": p99,
        "index_throughput": throughput,
        "peak_memory": peak_memory,
        "error_rate": 0.0,
    }


def evaluate(
    metrics: dict[str, float],
    thresholds: dict[str, float],
    waivers: dict[str, dict[str, str]],
) -> int:
    checks = {
        "query_p50": metrics["query_p50"] <= thresholds["query_p50"],
        "query_p95": metrics["query_p95"] <= thresholds["query_p95"],
        "query_p99": metrics["query_p99"] <= thresholds["query_p99"],
        "index_throughput": metrics["index_throughput"] >= thresholds["index_throughput"],
        "peak_memory": metrics["peak_memory"] <= thresholds["peak_memory"],
        "error_rate": metrics["error_rate"] <= thresholds["error_rate"],
    }

    print("Benchmark threshold evaluation")
    suppressed: list[str] = []
    for key in (
        "query_p50",
        "query_p95",
        "query_p99",
        "index_throughput",
        "peak_memory",
        "error_rate",
    ):
        waived = not checks[key] and key in waivers
        status = "PASS" if checks[key] else ("WAIVED" if waived else "FAIL")
        if waived:
            waiver = waivers[key]
            suppressed.append(key)
            print(
                f"- {key}: value={metrics[key]:.4f} threshold={thresholds[key]:.4f} status={status} "
                f"owner={waiver['owner']} issue={waiver['issue']} expiry={waiver['expiry']}"
            )
            continue
        print(
            f"- {key}: value={metrics[key]:.4f} threshold={thresholds[key]:.4f} status={status}"
        )

    if all(checks.values()) or all(checks[key] or key in waivers for key in checks):
        if suppressed:
            print(f"Threshold failures waived for metrics: {', '.join(sorted(suppressed))}")
        print("All thresholds satisfied")
        return 0

    print("One or more thresholds failed")
    return 1


def main() -> int:
    args = parse_args()
    try:
        thresholds = load_thresholds(Path(args.thresholds))
        metrics = build_metrics(args)
        waivers_path = Path(args.waivers) if args.waivers else None
        waivers = load_active_waivers(waivers_path, set(thresholds))
        return evaluate(metrics, thresholds, waivers)
    except Exception as exc:  # pragma: no cover - intentional CLI error surface
        print(f"error: {exc}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
