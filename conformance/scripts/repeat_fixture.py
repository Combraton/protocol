#!/usr/bin/env python3
"""Run one fixture repeatedly and record whether every run has the same, intended outcome.

Used for deterministic concurrency regressions (decision 007, REL-13): the correct provider must
pass every run, and the mutant that restores the race must fail every run at the expected step
with the expected reason. Any other outcome, in any run, fails the check.

Usage (from the repository root, after `cargo build`):
  python3 conformance/scripts/repeat_fixture.py --participant conformance/participants/reference-provider-unix.json \\
      --fixture socket.subscription-recheck-race-regression --runs 20 --out conformance/results/race-repeat/correct
  python3 conformance/scripts/repeat_fixture.py --participant conformance/participants/reference-provider-unix.json \\
      --fixture socket.subscription-recheck-race-regression --mutant recheck-outside-lock --runs 20 \\
      --out conformance/results/race-repeat/recheck-outside-lock
"""
import argparse
import json
import os
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--runner", default="target/debug/combraton-conformance")
    parser.add_argument("--participant", required=True)
    parser.add_argument("--fixture", required=True)
    parser.add_argument("--mutant")
    parser.add_argument("--runs", type=int, default=20)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    fixture_path = None
    for root, _, files in os.walk("conformance/fixtures"):
        for name in files:
            if name.endswith(".json"):
                with open(os.path.join(root, name)) as handle:
                    if json.load(handle).get("id") == args.fixture:
                        fixture_path = os.path.join(root, name)
    if fixture_path is None:
        print(f"repeat-fixture: no fixture {args.fixture}")
        return 2
    with open(fixture_path) as handle:
        fixture = json.load(handle)
    expectation = fixture.get("kill_expectations", {}).get(args.mutant) if args.mutant else None
    if args.mutant and expectation is None:
        print(f"repeat-fixture: {args.fixture} states no kill_expectations for {args.mutant}")
        return 2
    runs = []
    for index in range(args.runs):
        out = os.path.join(args.out, f"run-{index + 1}")
        command = [args.runner, "run", "--participant", args.participant, "--out", out, "--filter", args.fixture]
        if args.mutant:
            command += ["--mutant", args.mutant]
        subprocess.run(command, capture_output=True, text=True)
        with open(os.path.join(out, "manifest.json")) as handle:
            case = next(r for r in json.load(handle)["results"] if r["fixture"] == args.fixture)
        if args.mutant:
            intended = (case["outcome"] in ("fail", "timeout") and case["failed_step"] == expectation["step"]
                        and expectation["reason_contains"] in (case["reason"] or ""))
        else:
            intended = case["outcome"] == "pass"
        runs.append({"run": index + 1, "outcome": case["outcome"], "failed_step": case["failed_step"],
                     "reason": case["reason"], "intended": intended})
    summary = {"format": "combraton-repeat-fixture/1", "fixture": args.fixture, "participant": args.participant,
               "mutant": args.mutant, "expected": expectation or {"outcome": "pass"}, "runs": len(runs),
               "intended_runs": sum(r["intended"] for r in runs), "all_intended": all(r["intended"] for r in runs),
               "results": runs}
    os.makedirs(args.out, exist_ok=True)
    with open(os.path.join(args.out, "summary.json"), "w") as handle:
        json.dump(summary, handle, indent=2)
        handle.write("\n")
    label = f"{args.fixture}" + (f" with mutant {args.mutant}" if args.mutant else "")
    print(f"repeat-fixture: {label}: {summary['intended_runs']}/{summary['runs']} runs as intended")
    return 0 if summary["all_intended"] else 1


if __name__ == "__main__":
    sys.exit(main())
