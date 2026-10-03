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
are keyed by the target-file path of each import edge.

GRANULARITY (v1.2, #167): the script reads one edge class at a time.
  --granularity namespace (default; v1.1 semantics unchanged): edges of kind
      `imports` — one using-line maps to ONE namespace-representative file via
      a sorted-first resolver. The representative caveat below applies HERE.
  --granularity type: edges of kind `type_imports` — one using-line maps to one
      edge PER referenced type. The dependency KEY is the TYPE SYMBOL
      (`type_ref.namespace.type_ref.name`; empty ns → bare name), NOT the file:
      two types packaged in one declaring file are DISTINCT dependencies, and a
      partial type spanning two files is ONE dependency (review P0-1). A type
      edge WITHOUT a type_ref (or with an empty name) is an ERROR, not a file
      fallback (tur-3 P1-2 — the symbol is required). Representative
      semantics is gone at this granularity: adding a lexicographically earlier
      file to a namespace does NOT shift type-level keys, so the v1.1
      "removed + new for the same semantic dependency" artifact disappears.
Partition semantics (below) are granularity-independent and frozen at v1.1.

TERMINOLOGY (frozen; see dogfood/neighborhood-ledger.jsonl amendment): the
counted quantity is *neighborhood edge multiplication* — represented
dependency multiplicity in the import-graph representation, at the selected
granularity. It is NOT a claim about architectural complexity.

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
v1.2 amendment (recorded in dogfood/neighborhood-ledger.jsonl): the v1.1
finding "mapping is deterministic per analysis ... but NOT an analyzer
invariant" is now scoped to the namespace granularity; #167 type-level edges
are the structural fix and v1.2 exposes them via --granularity type.

Output: JSON record (schema neighborhood-accounting-v1.2) and a markdown
table on stdout. Companion to dogfood/ledger.jsonl — a separate measurement
stratum; it does NOT edit finalized ledger lines.

Usage:
  py scripts/neighborhood_accounting.py \
    --before dogfood/runs/<run>/baseline.json \
    --after  dogfood/runs/<run>/after.json \
    --nodes  path/to/Target.cs,path/to/NewBoundary.cs \
    [--granularity {namespace,type}] \
    [--out   dogfood/runs/<run>/neighborhood.json] \
  py scripts/neighborhood_accounting.py --check
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

SCHEMA = "neighborhood-accounting-v1.2"

GRANULARITY_KINDS = {
    "namespace": "imports",
    "type": "type_imports",
}


def load_snapshot(
    path: str, granularity: str = "namespace"
) -> tuple[dict[int, str], dict[int, list[str]], dict[int, dict]]:
    kind = GRANULARITY_KINDS[granularity]
    with open(path, encoding="utf-8") as f:
        d = json.load(f)
    id_to_path = {n["node_id"]: n["path"] for n in d["nodes"]}
    metrics = {n["node_id"]: n for n in d["nodes"]}
    out_edges: dict[int, list[str]] = {}
    for e in d["edges"]:
        if e["from"] == e["to"] or e.get("is_type_only"):
            continue
        if e.get("kind") != kind:
            continue
        out_edges.setdefault(e["from"], []).append(
            _dep_key(e, id_to_path, granularity)
        )
    return id_to_path, out_edges, metrics


def _dep_key(e: dict, id_to_path: dict[int, str], granularity: str) -> str:
    """Dependency key at the selected granularity.

    namespace -> target file path (v1.1 semantics). type -> TYPE SYMBOL
    (ns.Name; bare name for global-ns types) — REQUIRED: a type edge without
    type_ref, or with an empty name, raises (identity IS the symbol; the file
    fallback would re-collapse same-file types — review tur-3 P1-2).
    """
    if granularity == "type":
        tref = e.get("type_ref")
        if not tref or not tref.get("name"):
            # Review tur-3 P1-2: symbol identity MISSING is an ERROR, not a
            # file fallback. Falling back to the target path would re-collapse
            # two types packaged in one file — exactly the P0-1 collapse this
            # instrument exists to separate. Empty namespace (global ns) is
            # valid; an empty NAME is not (identity IS the name).
            raise ValueError(
                f"type-granularity edge {e['from']}->{e['to']} lacks symbol identity "
                "(type_ref missing or type_ref.name empty) — in type granularity the "
                "type symbol is REQUIRED; file-path fallback removed (#167 review tur-3)"
            )
        ns, name = tref.get("namespace", ""), tref["name"]
        return f"{ns}.{name}" if ns else name
    return id_to_path[e["to"]]


def holder_map(
    neighborhood: list[str],
    id_to_path: dict[int, str],
    out_edges: dict[int, list[int]],
) -> dict[str, set[str]]:
    """deps: dependency key (ns-representative OR type file, per granularity)
    -> set of holder node paths."""
    deps: dict[str, set[str]] = {}
    wanted = set(neighborhood)
    for nid, npath in id_to_path.items():
        if npath not in wanted:
            continue
        for key in out_edges.get(nid, []):
            deps.setdefault(key, set()).add(npath)
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
    granularity: str = "namespace",
) -> dict[str, dict[str, float | None]]:
    """Per-holder coupling at the selected granularity.

    namespace → node `coupling` (x, unchanged since v1.1); type → node
    `coupling_type` (x_type, #167) when present, else None (language/file
    without type-level resolution).
    """
    field = "coupling" if granularity == "namespace" else "coupling_type"
    result: dict[str, dict[str, float | None]] = {}
    for label, ids, metrics in snapshots:
        for nid, npath in ids.items():
            if npath in set(neighborhood):
                raw = metrics[nid].get(field)
                result.setdefault(npath, {})[label] = (
                    round(raw["value"], 6) if raw is not None else None
                )
    for npath, labels in result.items():  # absent side -> null
        for label, _, _ in snapshots:
            labels.setdefault(label, None)
    return result


def run(args: argparse.Namespace) -> int:
    neighborhood = [p.strip() for p in args.nodes.split(",") if p.strip()]
    b_ids, b_edges, b_metrics = load_snapshot(args.before, args.granularity)
    a_ids, a_edges, a_metrics = load_snapshot(args.after, args.granularity)
    record = account(neighborhood, b_ids, b_edges, a_ids, a_edges)
    record = {
        "schema_version": SCHEMA,
        "granularity": args.granularity,
        "edge_kind": GRANULARITY_KINDS[args.granularity],
        "before_snapshot": str(Path(args.before)),
        "after_snapshot": str(Path(args.after)),
        "neighborhood": neighborhood,
        **record,
        "node_coupling": node_couplings(
            neighborhood,
            [("before", b_ids, b_metrics), ("after", a_ids, a_metrics)],
            args.granularity,
        ),
    }
    if args.out:
        with open(args.out, "w", encoding="utf-8", newline="\n") as f:
            json.dump(record, f, indent=1, ensure_ascii=False)
            f.write("\n")
        print(f"written: {args.out}")

    s = record["summary"]
    key_header = "dependency (ns-representative)" if args.granularity == "namespace" \
        else "dependency (type file)"
    print(f"\nneighborhood ({len(neighborhood)} nodes, granularity={args.granularity})")
    print(f"  dependency union: {s['dependency_union_before']} -> {s['dependency_union_after']}"
          f"   (new entering: {len(s['new_dependencies_entering'])})")
    print(f"  out-edge sum:     {s['neighborhood_out_sum_before']} -> {s['neighborhood_out_sum_after']}")
    print(f"  classes: {json.dumps(s['classes'])}")
    print(f"\n| {key_header} | cardinality | ownership | class | before | after |")
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
    # Edges are DEPENDENCY-KEYED (load_snapshot/_dep_key contract): here,
    # representative file paths (namespace-granularity fixture).
    # before: T holds A,B,C,D,G(shared with G2); after: F(new) holds C,D,E; T keeps B,D; G2 keeps G
    b_edges = {1: [A, B, C, D, G], 2: [G]}
    a_edges = {1: [B, D], 2: [G], 4: [C, D, E]}
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

    # --- v1.2 (#167): granularity fixtures — ns-representative vs type-file keys ---
    import tempfile

    NA, NB = "svc/HolderA.cs", "svc/HolderB.cs"
    NL = "svc/Ledger.cs"

    def write_snapshot(nodes: dict[int, str], edges: list[dict]) -> str:
        tmp = tempfile.NamedTemporaryFile(
            "w", suffix=".json", delete=False, encoding="utf-8"
        )
        json.dump(
            {
                "schema_version": 2,
                "nodes": [
                    {"node_id": nid, "path": p, "coupling": {"value": 0.5}} for nid, p in nodes.items()
                ],
                "edges": edges,
            },
            tmp,
        )
        tmp.close()
        return tmp.name

    # (ii) S2a~=S2b disambiguation: two holders reference DIFFERENT types from
    # the same namespace. ns-mode: ONE representative key held by both (looks
    # shared). type-mode: symbol keys, one holder each (not shared at all).
    # P0-1: FraudEvent + ChargebackEvent AYNI dosyada (10) — dosya-anahtarlı
    # muhasebe ikisini çökertirdi; sembol anahtarlar ayırır (type union 3).
    nodes = {1: NA, 2: NB, 10: "ev/FraudEvent.cs", 11: "ev/RefundEvent.cs", 90: "ev/Rep.cs"}
    edges = [
        {"from": 1, "to": 90, "kind": "imports"},
        {"from": 2, "to": 90, "kind": "imports"},
        {"from": 1, "to": 10, "kind": "type_imports",
         "type_ref": {"namespace": "ev", "name": "FraudEvent"}},
        {"from": 1, "to": 10, "kind": "type_imports",
         "type_ref": {"namespace": "ev", "name": "ChargebackEvent"}},
        {"from": 2, "to": 11, "kind": "type_imports",
         "type_ref": {"namespace": "ev", "name": "RefundEvent"}},
    ]
    path = write_snapshot(nodes, edges)
    ns_ids, ns_edges, _ = load_snapshot(path, "namespace")
    ty_ids, ty_edges, _ = load_snapshot(path, "type")
    ns_rec = account([NA, NB], ns_ids, ns_edges, ns_ids, ns_edges)
    ty_rec = account([NA, NB], ty_ids, ty_edges, ty_ids, ty_edges)
    if ns_rec["summary"]["dependency_union_before"] != 1:
        raise RuntimeError(f"ns union drift: {ns_rec['summary']}")
    if ty_rec["summary"]["dependency_union_before"] != 3:
        raise RuntimeError(f"type union drift (S2 disambiguation + same-file P0-1): {ty_rec['summary']}")
    if "ev.FraudEvent" not in ty_rec["per_dependency"] or "ev.ChargebackEvent" not in ty_rec["per_dependency"]:
        raise RuntimeError(f"type symbol keys missing: {sorted(ty_rec['per_dependency'])}")
    if ns_rec["summary"]["neighborhood_out_sum_before"] != 2:
        raise RuntimeError("ns out-sum drift")
    if ty_rec["summary"]["neighborhood_out_sum_before"] != 3:
        raise RuntimeError("type out-sum drift (P0-1: ayni dosya 2 sembol = 2 kenar)")

    # (iv) representative-shift artifact: new lexicographically-earlier file in
    # the ns SHIFTS the ns key (removed+new) but type keys are stable (unchanged).
    before = write_snapshot(
        {1: NA, 10: "ev/FraudEvent.cs", 91: "ev/OldRep.cs"},
        [
            {"from": 1, "to": 91, "kind": "imports"},
            {"from": 1, "to": 10, "kind": "type_imports",
             "type_ref": {"namespace": "ev", "name": "FraudEvent"}},
        ],
    )
    after = write_snapshot(
        {1: NA, 10: "ev/FraudEvent.cs", 92: "ev/AaaRep.cs"},
        [
            {"from": 1, "to": 92, "kind": "imports"},
            {"from": 1, "to": 10, "kind": "type_imports",
             "type_ref": {"namespace": "ev", "name": "FraudEvent"}},
        ],
    )
    b_ids2, b_edges2, _ = load_snapshot(before, "namespace")
    a_ids2, a_edges2, _ = load_snapshot(after, "namespace")
    ns_shift = account([NA], b_ids2, b_edges2, a_ids2, a_edges2)
    if ns_shift["summary"]["classes"]["removed"] != 1 or ns_shift["summary"]["classes"]["new"] != 1:
        raise RuntimeError(f"ns representative-shift fixture drift: {ns_shift['summary']}")
    b_ids3, b_edges3, _ = load_snapshot(before, "type")
    a_ids3, a_edges3, _ = load_snapshot(after, "type")
    ty_stable = account([NA], b_ids3, b_edges3, a_ids3, a_edges3)
    if ty_stable["summary"]["classes"] != {
        "removed": 0, "moved": 0, "multiplied": 0, "unchanged": 1, "new": 0, "reduced": 0
    }:
        raise RuntimeError(f"type stability drift: {ty_stable['summary']}")

    # (iii)+(i) type-level moved / true multiplied: same type key, holder set
    # changes {A}->{L} → moved; {A}->{A,B} → multiplied.
    bm = write_snapshot(
        {1: NA, 10: "ev/FraudEvent.cs"},
        [{"from": 1, "to": 10, "kind": "type_imports",
          "type_ref": {"namespace": "ev", "name": "FraudEvent"}}],
    )
    am_moved = write_snapshot(
        {5: NL, 10: "ev/FraudEvent.cs"},
        [{"from": 5, "to": 10, "kind": "type_imports",
          "type_ref": {"namespace": "ev", "name": "FraudEvent"}}],
    )
    am_mult = write_snapshot(
        {1: NA, 2: NB, 10: "ev/FraudEvent.cs"},
        [
            {"from": 1, "to": 10, "kind": "type_imports",
             "type_ref": {"namespace": "ev", "name": "FraudEvent"}},
            {"from": 2, "to": 10, "kind": "type_imports",
             "type_ref": {"namespace": "ev", "name": "FraudEvent"}},
        ],
    )
    bM, eM, _ = load_snapshot(bm, "type")
    aL, eL, _ = load_snapshot(am_moved, "type")
    moved_rec = account([NA, NL], bM, eM, aL, eL)
    if moved_rec["per_dependency"]["ev.FraudEvent"]["class"] != "moved":
        raise RuntimeError(f"type moved drift: {moved_rec['per_dependency']}")
    aP, eP, _ = load_snapshot(am_mult, "type")
    mult_rec = account([NA, NB], bM, eM, aP, eP)
    if mult_rec["per_dependency"]["ev.FraudEvent"]["class"] != "multiplied":
        raise RuntimeError(f"type true-multiplication drift: {mult_rec['per_dependency']}")

    # (v) tur-3 P1-2 fail-closed: sembol kimligi EKSIK tip kenari (ref'siz ya
    # da bos isim) hata verir — path fallback KALDIRILDI (ayni dosyadaki iki
    # tipi yeniden cokertmesin diye; P0-1 ontolojisi).
    for bad_edge in (
        {"from": 1, "to": 10, "kind": "type_imports"},  # type_ref yok
        {"from": 1, "to": 10, "kind": "type_imports",
         "type_ref": {"namespace": "ev", "name": ""}},   # bos isim
    ):
        bad = write_snapshot({1: NA, 10: "ev/FraudEvent.cs"}, [bad_edge])
        try:
            load_snapshot(bad, "type")
        except ValueError as exc:
            if "symbol" not in str(exc):
                raise RuntimeError(f"fail-closed mesaj drift: {exc}")
        else:
            raise RuntimeError(f"eksik sembol kimligi red edilmeli: {bad_edge}")
        # namespace kipinde ayni kenar sorun degil (kind filtresi disarida birakar).
        load_snapshot(bad, "namespace")

    print("self-check PASS (v1.2 granularity fixtures: S2 disambiguation + same-file"
          " P0-1 symbol keys (ns union 1 vs type union 3); representative-shift"
          " artifact ns-only; type-symbol moved/multiplied; missing-symbol"
          " fail-closed (no file fallback)")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--before", help="baseline.json space snapshot")
    ap.add_argument("--after", help="after.json space snapshot")
    ap.add_argument("--nodes", help="comma-separated neighborhood node paths")
    ap.add_argument("--granularity", choices=sorted(GRANULARITY_KINDS), default="namespace",
                    help="edge class to account: namespace (v1.1 semantics, kind=imports)"
                         " or type (#167 type-level edges, kind=type_imports)")
    ap.add_argument("--out", help="optional path for the JSON record")
    ap.add_argument("--check", action="store_true",
                    help="run embedded instrument self-check fixtures (behavioral sanity;"
                         " the freeze itself is the commit SHA — see instrument contract)")
    args = ap.parse_args()
    if args.check:
        return self_check()
    if not (args.before and args.after and args.nodes):
        ap.error("--before, --after and --nodes are required (or use --check)")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
