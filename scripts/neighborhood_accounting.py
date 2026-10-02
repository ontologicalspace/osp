#!/usr/bin/env python3
"""Neighborhood dependency accounting for OSP dogfood runs.

Answers the question a node-local coupling metric cannot: for a run's
neighborhood (target node + nodes created by the accepted proposal), was each
dependency REMOVED from the OBSERVED NEIGHBORHOOD, MOVED to exactly one other
holder, UNCHANGED, or MULTIPLIED across several holders? Scope: holders are
counted only inside the neighborhood — "removed" means removed from the
observed neighborhood and "new" means newly entering it; the dependency may
still exist elsewhere in the graph. This is not a system-removal claim.

Inputs are two `osp analyze` space snapshots of the SAME repository (before
and after the promoted patch) plus the neighborhood node paths. Dependencies
are keyed by the representative target-file path of each import edge (OSP's
import graph maps one using-line to one representative file via a
sorted-first namespace resolver). The mapping is deterministic per analysis
and empirically stable for runs 9-11, but it is NOT an analyzer invariant:
adding a lexicographically earlier file to a namespace can shift the
representative path, which would surface here as `removed + new` for the same
semantic dependency. #167 (type-level edges) is the structural fix.

TERMINOLOGY (frozen; see dogfood/neighborhood-ledger.jsonl amendment): the
counted quantity is *neighborhood edge multiplication* — represented
dependency multiplicity in the import-graph representation. It is NOT a claim
about architectural complexity: until type-level edges (#167) land, edge
identity is namespace-representative, not a semantic dependency.

PARTITION SEMANTICS (frozen at v1.1):
For each dependency d with before-owner set B_d and after-owner set A_d:

    cardinality(B, A) in {new, removed, same, increased, decreased}
        new        B = {} and A != {}
        removed    B != {} and A = {}
        same       B != {} and A != {} and |A| = |B|
        increased  |A| > |B|
        decreased  0 < |A| < |B|
    ownership(B, A) in {unchanged, changed}      (set equality)

    label = f(cardinality, ownership):
        new -> new;  removed -> removed
        (same, unchanged) -> unchanged
        (same, changed)   -> moved
        increased -> multiplied
        decreased -> reduced

The six labels are mutually exclusive and exhaustive (trichotomy on |A| vs
|B| once the empty-set cases are settled). The orthogonal cardinality x
ownership pair is recorded per dependency because a single label can conflate
facts: owner change can co-occur with growth ((increased, changed) reports
both; the label alone says only "multiplied").

INSTRUMENT CONTRACT (freeze semantics — two distinct things):
  instrument identity = the merge/instrument commit SHA. The commit is what
      freezes the algorithm; a later edit requires a new schema_version.
  behavioral sanity   = `--check`. Embedded fixtures assert every
      classification and the label = f(cardinality, ownership) derivation.
      This is a regression self-test, NOT a freeze: if algorithm and fixtures
      change together, or behavior outside fixture coverage changes,
      `--check` still passes.
Probe contract: the paired adversarial probe (dogfood/adversarial-probe-
tasarim.md) must run THIS script AS RECORDED in its instrument_commit
(e.g. `git show <sha>:scripts/neighborhood_accounting.py`), not whatever
later lives on main. Writing the SHA into the artifact while running a newer
script is not a freeze.

Output: JSON record (schema neighborhood-accounting-v1.1) and a markdown
table on stdout. Companion to dogfood/ledger.jsonl — a separate measurement
stratum; it does NOT edit finalized ledger lines.

Usage:
  py scripts/neighborhood_accounting.py \
      --before dogfood/runs/<run>/baseline.json \
      --after  dogfood/runs/<run>/after.json \
      --nodes  path/to/Target.cs,path/to/NewBoundary.cs \
      [--out   dogfood/runs/<run>/neighborhood.json] \
  py scripts/neighborhood_accounting.py --check
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

SCHEMA = "neighborhood-accounting-v1.1"


def load_snapshot(path: str) -> tuple[dict[int, str], dict[int, list[int]], dict[int, dict]]:
    with open(path, encoding="utf-8") as f:
        d = json.load(f)
    id_to_path = {n["node_id"]: n["path"] for n in d["nodes"]}
    metrics = {n["node_id"]: n for n in d["nodes"]}
    out_edges: dict[int, list[int]] = {}
    for e in d["edges"]:
        if e["from"] == e["to"] or e.get("is_type_only"):
            continue
        if e.get("kind") != "imports":
            continue
        out_edges.setdefault(e["from"], []).append(e["to"])
    return id_to_path, out_edges, metrics


def holder_map(
    neighborhood: list[str],
    id_to_path: dict[int, str],
    out_edges: dict[int, list[int]],
) -> dict[str, set[str]]:
    """deps: representative target path -> set of holder node paths."""
    deps: dict[str, set[str]] = {}
    wanted = set(neighborhood)
    for nid, npath in id_to_path.items():
        if npath not in wanted:
            continue
        for tid in out_edges.get(nid, []):
            deps.setdefault(id_to_path[tid], set()).add(npath)
    return deps


def cardinality(before_h: set[str], after_h: set[str]) -> str:
    if not before_h and after_h:
        return "new"
    if before_h and not after_h:
        return "removed"
    if len(after_h) == len(before_h):
        return "same"
    return "increased" if len(after_h) > len(before_h) else "decreased"


def ownership(before_h: set[str], after_h: set[str]) -> str:
    return "unchanged" if before_h == after_h else "changed"


def label_from(card: str, own: str) -> str:
    direct = {
        "new": "new",
        "removed": "removed",
        "increased": "multiplied",
        "decreased": "reduced",
    }
    if card in direct:
        return direct[card]
    if card != "same":
        raise ValueError(f"unknown cardinality: {card}")
    return "unchanged" if own == "unchanged" else "moved"


def classify(before_h: set[str], after_h: set[str]) -> str:
    return label_from(cardinality(before_h, after_h), ownership(before_h, after_h))


def account(
    neighborhood: list[str],
    b_ids: dict[int, str],
    b_edges: dict[int, list[int]],
    a_ids: dict[int, str],
    a_edges: dict[int, list[int]],
) -> dict:
    b_paths = set(b_ids.values())
    a_paths = set(a_ids.values())
    absent = [p for p in neighborhood if p not in b_paths and p not in a_paths]
    if absent:
        raise ValueError(f"nodes not found in either snapshot: {absent}")

    b_deps = holder_map(neighborhood, b_ids, b_edges)
    a_deps = holder_map(neighborhood, a_ids, a_edges)

    per_dep = {}
    for key in sorted(set(b_deps) | set(a_deps)):
        bh = b_deps.get(key, set())
        ah = a_deps.get(key, set())
        per_dep[key] = {
            "before": sorted(bh),
            "after": sorted(ah),
            "cardinality": cardinality(bh, ah),
            "ownership": ownership(bh, ah),
            "class": classify(bh, ah),
        }

    # Deterministic shape for frozen artifacts: all six labels always present
    # (missing != zero must never be ambiguous downstream).
    classes: dict[str, int] = {k: 0 for k in
                               ("removed", "moved", "multiplied", "unchanged", "new", "reduced")}
    for v in per_dep.values():
        classes[v["class"]] += 1

    def out_sum(deps: dict[str, set[str]], holders: set[str]) -> int:
        return sum(len(h & holders) for h in deps.values())

    b_holders = {p for p in neighborhood if p in b_paths}
    a_holders = {p for p in neighborhood if p in a_paths}

    return {
        "summary": {
            "dependency_union_before": len(b_deps),
            "dependency_union_after": len(a_deps),
            "new_dependencies_entering": sorted(set(a_deps) - set(b_deps)),
            "classes": classes,
            "neighborhood_out_sum_before": out_sum(b_deps, b_holders),
            "neighborhood_out_sum_after": out_sum(a_deps, a_holders),
        },
        "per_dependency": per_dep,
    }


def node_couplings(
    neighborhood: list[str],
    snapshots: list[tuple[str, dict[int, str], dict[int, dict]]],
) -> dict[str, dict[str, float | None]]:
    result: dict[str, dict[str, float | None]] = {}
    for label, ids, metrics in snapshots:
        for nid, npath in ids.items():
            if npath in set(neighborhood):
                result.setdefault(npath, {})[label] = round(metrics[nid]["coupling"]["value"], 6)
    for npath, labels in result.items():  # absent side -> null
        for label, _, _ in snapshots:
            labels.setdefault(label, None)
    return result


def run(args: argparse.Namespace) -> int:
    neighborhood = [p.strip() for p in args.nodes.split(",") if p.strip()]
    b_ids, b_edges, b_metrics = load_snapshot(args.before)
    a_ids, a_edges, a_metrics = load_snapshot(args.after)
    record = account(neighborhood, b_ids, b_edges, a_ids, a_edges)
    record = {
        "schema_version": SCHEMA,
        "before_snapshot": str(Path(args.before)),
        "after_snapshot": str(Path(args.after)),
        "neighborhood": neighborhood,
        **record,
        "node_coupling": node_couplings(
            neighborhood,
            [("before", b_ids, b_metrics), ("after", a_ids, a_metrics)],
        ),
    }
    if args.out:
        with open(args.out, "w", encoding="utf-8", newline="\n") as f:
            json.dump(record, f, indent=1, ensure_ascii=False)
            f.write("\n")
        print(f"written: {args.out}")

    s = record["summary"]
    print(f"\nneighborhood ({len(neighborhood)} nodes)")
    print(f"  dependency union: {s['dependency_union_before']} -> {s['dependency_union_after']}"
          f"   (new entering: {len(s['new_dependencies_entering'])})")
    print(f"  out-edge sum:     {s['neighborhood_out_sum_before']} -> {s['neighborhood_out_sum_after']}")
    print(f"  classes: {json.dumps(s['classes'])}")
    print("\n| dependency (representative) | cardinality | ownership | class | before | after |")
    print("|---|---|---|---|---|---|")
    for key, v in record["per_dependency"].items():
        short = key.split("src/")[-1] if "src/" in key else key
        print(f"| {short} | {v['cardinality']} | {v['ownership']} | {v['class']} |"
              f" {len(v['before'])} | {len(v['after'])} |")
    for npath, c in record["node_coupling"].items():
        print(f"  c {npath.split('/')[-1]}: {c.get('before')} -> {c.get('after')}")
    return 0


def self_check() -> int:
    """Embedded fixtures covering every classification and the partition
    derivation. Behavioral sanity test (NOT the freeze itself — instrument
    identity is the commit SHA; see the instrument contract in the module
    docstring). Explicit checks, not `assert`: python -O strips asserts and
    this must still fail loudly."""
    T, G2, F = "pkg/T.cs", "pkg/G2.cs", "pkg/F.cs"
    A, B, C, D, E, G = "dep/A.cs", "dep/B.cs", "dep/C.cs", "dep/D.cs", "dep/E.cs", "dep/G.cs"
    b_ids = {1: T, 2: G2, 10: A, 11: B, 12: C, 13: D, 14: G}
    a_ids = {1: T, 2: G2, 4: F, 10: A, 11: B, 12: C, 13: D, 14: G, 15: E}
    # before: T holds A,B,C,D,G(shared with G2); after: F(new) holds C,D,E; T keeps B,D; G2 keeps G
    b_edges = {1: [10, 11, 12, 13, 14], 2: [14]}
    a_edges = {1: [11, 13], 2: [14], 4: [12, 13, 15]}
    rec = account([T, G2, F], b_ids, b_edges, a_ids, a_edges)
    per = {k.split("/")[-1]: v for k, v in rec["per_dependency"].items()}
    expected = {
        "A.cs": ("removed", "changed", "removed"),
        "B.cs": ("same", "unchanged", "unchanged"),
        "C.cs": ("same", "changed", "moved"),
        "D.cs": ("increased", "changed", "multiplied"),
        "E.cs": ("new", "changed", "new"),
        "G.cs": ("decreased", "changed", "reduced"),
    }
    for name, (card, own, label) in expected.items():
        got = (per[name]["cardinality"], per[name]["ownership"], per[name]["class"])
        if got != (card, own, label):
            raise RuntimeError(f"partition drift at {name}: {got} != {(card, own, label)}")
        if per[name]["class"] != label_from(per[name]["cardinality"], per[name]["ownership"]):
            raise RuntimeError(f"label derivation drift at {name}")
    s = rec["summary"]
    if s["neighborhood_out_sum_before"] != 6 or s["neighborhood_out_sum_after"] != 6:
        raise RuntimeError(f"out-sum drift: {s}")
    if s["dependency_union_before"] != 5 or s["dependency_union_after"] != 5:
        raise RuntimeError(f"union drift: {s}")
    if set(s["classes"].keys()) != {"removed", "moved", "multiplied", "unchanged", "new", "reduced"}:
        raise RuntimeError(f"classes shape drift: {s['classes']}")
    print("self-check PASS (six-class fixtures + cardinality/ownership cross-derivation;"
          " all-six-key classes; out-sum 6->6; union 5->5)")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--before", help="baseline.json space snapshot")
    ap.add_argument("--after", help="after.json space snapshot")
    ap.add_argument("--nodes", help="comma-separated neighborhood node paths")
    ap.add_argument("--out", help="optional path for the JSON record")
    ap.add_argument("--check", action="store_true", help="run embedded freeze-guard fixtures")
    args = ap.parse_args()
    if args.check:
        return self_check()
    if not (args.before and args.after and args.nodes):
        ap.error("--before, --after and --nodes are required (or use --check)")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
