#!/usr/bin/env -S uv run --script
# 
# /// script
# requires-python = ">=3.12"
# ///

"""
Compare two llvm-cov export JSON snapshots to measure coverage improvement.
Usage: uv run --script compare_coverage.py snapshot_old.json snapshot_new.json
"""

import json
import sys


def load_snapshot(path):
    with open(path) as f:
        return json.load(f)


def get_totals(snapshot):
    return snapshot["data"][0]["totals"]


def get_files(snapshot):
    return {f["filename"]: f["summary"] for f in snapshot["data"][0]["files"]}


def pct(covered, total):
    return (covered / total * 100) if total > 0 else 0.0


def delta_str(old, new):
    diff = new - old
    sign = "+" if diff >= 0 else ""
    return f"{sign}{diff:.2f}%"


def print_section(title, old, new, key):
    o_cov = old[key]["covered"]
    o_tot = old[key]["count"]
    n_cov = new[key]["covered"]
    n_tot = new[key]["count"]

    o_pct = pct(o_cov, o_tot)
    n_pct = pct(n_cov, n_tot)

    print(f"  {title:<12} {o_pct:6.2f}%  ->  {n_pct:6.2f}%   {delta_str(o_pct, n_pct)}")


def main():
    if len(sys.argv) != 3:
        print("Usage: compare_coverage.py <old.json> <new.json>")
        sys.exit(1)

    old_path, new_path = sys.argv[1], sys.argv[2]
    old = load_snapshot(old_path)
    new = load_snapshot(new_path)

    old_totals = get_totals(old)
    new_totals = get_totals(new)

    print(f"\n{'='*55}")
    print("  Coverage comparison")
    print(f"  Old: {old_path}")
    print(f"  New: {new_path}")
    print(f"{'='*55}")
    print(f"  {'Metric':<12} {'Old':>8}    {'New':>8}   {'Delta':>8}")
    print(f"{'-'*55}")
    print_section("Lines",    old_totals, new_totals, "lines")
    print_section("Branches", old_totals, new_totals, "branches")
    print_section("Regions",  old_totals, new_totals, "regions")
    print_section("Functions",old_totals, new_totals, "functions")
    print(f"{'='*55}\n")

    # Per-file breakdown — show files that improved or regressed
    old_files = get_files(old)
    new_files = get_files(new)

    all_files = sorted(set(old_files) | set(new_files))

    improved = []
    regressed = []
    unchanged = []

    for fname in all_files:
        if fname not in old_files or fname not in new_files:
            continue
        o = pct(old_files[fname]["lines"]["covered"], old_files[fname]["lines"]["count"])
        n = pct(new_files[fname]["lines"]["covered"], new_files[fname]["lines"]["count"])
        diff = n - o
        short = fname.split("/src/")[-1] if "/src/" in fname else fname
        if diff > 0.01:
            improved.append((short, o, n, diff))
        elif diff < -0.01:
            regressed.append((short, o, n, diff))
        else:
            unchanged.append(short)

    if improved:
        print(f"  Files with improved coverage ({len(improved)}):")
        for name, o, n, d in sorted(improved, key=lambda x: -x[3]):
            print(f"    {name:<45} {o:6.2f}% -> {n:6.2f}%  (+{d:.2f}%)")
        print()

    if regressed:
        print(f"  Files with regressed coverage ({len(regressed)}):")
        for name, o, n, d in sorted(regressed, key=lambda x: x[3]):
            print(f"    {name:<45} {o:6.2f}% -> {n:6.2f}%  ({d:.2f}%)")
        print()

    print(f"  Unchanged files: {len(unchanged)}")
    print()


if __name__ == "__main__":
    main()
