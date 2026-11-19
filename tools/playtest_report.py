#!/usr/bin/env python3
"""Sums up the playtest log (logs/nights.csv next to the saves) like the
balancing simulation does, so real games can be compared with

    BEVY_ASSET_ROOT=. cargo run --release -- --simulate 500

Usage:
    python3 tools/playtest_report.py [path/to/nights.csv]

Without a path it looks in the usual save folder (or SCFS_SAVE_DIR).
"""

import csv
import os
import sys
from collections import defaultdict
from pathlib import Path


def default_csv():
    if os.environ.get("SCFS_SAVE_DIR"):
        base = Path(os.environ["SCFS_SAVE_DIR"])
    elif sys.platform == "win32":
        base = Path(os.environ.get("APPDATA", "")) / "SpareChangeForSpirits"
    elif sys.platform == "darwin":
        base = Path.home() / "Library/Application Support/SpareChangeForSpirits"
    else:
        data = os.environ.get("XDG_DATA_HOME") or str(Path.home() / ".local/share")
        base = Path(data) / "spare-change-for-spirits"
    return base / "logs" / "nights.csv"


def main():
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else default_csv()
    if not path.exists():
        sys.exit(f"no playtest log at {path}")
    rows = list(csv.DictReader(path.open(encoding="utf-8")))
    if not rows:
        sys.exit("the playtest log is empty")

    by_night = defaultdict(list)
    for r in rows:
        by_night[int(r["night"])].append(r)

    def avg(rs, key):
        return sum(float(r[key]) for r in rs) / len(rs)

    cols = [
        ("money", "money"), ("rep", "reputation"), ("visitors", "visitors"),
        ("sales", "sales"), ("revenue", "revenue"), ("declined", "declined"),
        ("wrong", "wrong_item"), ("fled", "fled"), ("unhappy", "unhappy"),
        ("faded", "faded"), ("crates", "crates"), ("counter s", "counter_avg_secs"),
        ("night s", "night_secs"),
    ]
    sessions = len({r["session"] for r in rows})
    print(f"{len(rows)} nights from {sessions} sessions in {path}\n")
    print(f"{'night':>5} {'played':>6} " + " ".join(f"{name:>9}" for name, _ in cols))
    for night in sorted(by_night):
        rs = by_night[night]
        print(f"{night:>5} {len(rs):>6} " + " ".join(f"{avg(rs, key):>9.1f}" for _, key in cols))

    total = lambda key: sum(float(r[key]) for r in rows)
    visitors = max(total("visitors"), 1.0)
    turns = total("counter_turns")
    counter = (
        sum(float(r["counter_avg_secs"]) * float(r["counter_turns"]) for r in rows) / turns
        if turns
        else 0.0
    )
    print()
    print(f"wishes fulfilled {100 * total('sales') / visitors:.0f}%"
          f" · fled {100 * total('fled') / visitors:.0f}%"
          f" · unhappy {100 * total('unhappy') / visitors:.0f}%"
          f" · change mistakes {total('change_wrong'):.0f} of {total('change_correct') + total('change_wrong'):.0f}")
    print(f"time per ghost at the counter: {counter:.1f} s"
          f" (the simulation assumes 9 s per sale, 3 s to turn a ghost away)")
    assists = sum(r["patient_ghosts"] == "true" or r["change_hint"] == "true" for r in rows)
    if assists:
        print(f"{assists} nights were played with an assist on (patient ghosts or change shown)")


if __name__ == "__main__":
    main()
