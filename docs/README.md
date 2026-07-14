# OSP Documentation Index

This directory contains the research artifacts, specifications, and results for the
**Ontological Space Protocol (OSP)** — a framework that models software projects as
navigable conceptual spaces with physics-like rules and BFT-inspired witnessing.

## Structure

### `papers/` — Three companion preprints

| Paper | File | Layer |
|---|---|---|
| Paper 1 | [`papers/paper1-static-space.md`](papers/paper1-static-space.md) | Static conceptual space (coordinates, BFT witnessing, metrics) |
| Paper 2 | [`papers/paper2-agent-trajectory.md`](papers/paper2-agent-trajectory.md) | Dynamic navigation (agent trajectory under measurement predicates) |
| Paper 3 | [`papers/paper3-concept-anchoring.md`](papers/paper3-concept-anchoring.md) | Genesis layer (human sentence → bound project work) |

All three are published on Zenodo:
- Paper 1: [10.5281/zenodo.21206545](https://doi.org/10.5281/zenodo.21206545)
- Paper 2: [10.5281/zenodo.21207704](https://doi.org/10.5281/zenodo.21207704)
- Paper 3 (latest): [10.5281/zenodo.21220992](https://doi.org/10.5281/zenodo.21220992) (concept DOI; resolves to the latest public version)
- Evidence Pack: [10.5281/zenodo.21207762](https://doi.org/10.5281/zenodo.21207762)

### `spec/` — Formal specifications

- [`spec/formalism.md`](spec/formalism.md) — Mathematical model (coordinate system, BFT proof, commit operator)
- [`spec/invariants.md`](spec/invariants.md) — Formal invariant spec (INV-T1..T8 trajectory + INV #1..#15 space)
- [`spec/mcp-design.md`](spec/mcp-design.md) — MCP server design (6 "never" principles, tool categories, INV matrix)

### `roadmap/` — Development roadmaps

- [`roadmap/paper2-roadmap.md`](roadmap/paper2-roadmap.md) — Paper 2 development roadmap (motivation, ontology, INV-T1..T8, §8 stage plan)
- [`roadmap/paper3-design.md`](roadmap/paper3-design.md) — Paper 3 design document (concept anchoring layers)

### `results/` — Empirical results and benchmarks

- [`results/scip-cohesion.md`](results/scip-cohesion.md) — Primary 15-repo corpus LCOM4 cohesion results
- [`results/corpus28.md`](results/corpus28.md) — Extended 23-repo results (Rust/Go cohesion + coupling + foam analysis)
- [`results/calibration-corpus.md`](results/calibration-corpus.md) — Corpus selection methodology
- [`results/calibration-results.md`](results/calibration-results.md) — Calibration run results
- [`results/token-benchmark.md`](results/token-benchmark.md) — Token-size benchmark (RQ5)
- [`results/literature-scan.md`](results/literature-scan.md) — Related work + originality analysis
- [`results/scale-bench-v3.md`](results/scale-bench-v3.md) — Scale benchmark v3
- [`results/spike-repos.md`](results/spike-repos.md) — Spike repository reference list

### `paper2-notes/` & `paper3-notes/` — Paper development notes

Stage-by-stage implementation notes, evidence files, and handoff documents from the
paper writing process. These are data-driven development artifacts.

- `paper2-notes/evidence/` — G2c corpus results (external repo analysis)
- `paper3-notes/evidence/` — Paper 3 frozen verification snapshots (Zenodo evidence pack source)
- `paper3-notes/evidence-pack/` — Zenodo evidence pack README + MANIFEST

### `notes/` — Out-of-scope design brainstorm

- [`notes/interaction-surfaces-design-document.md`](notes/interaction-surfaces-design-document.md) — DS1-DS3 interaction surface design (strictly out of Paper 3 scope, reference only)

### `archive/` — Superseded documents

Historical documents kept for reference but superseded by current versions:
- `HANDOFF-G2c5-and-beyond.md` — G2c-5 handoff (stage completed, STATUS.md is current)
- `spike-results-v2.md`, `spike-results-v3.md` — Earlier spike results

## Status

For current project status, see [`STATUS.md`](STATUS.md) (kept at docs root for visibility).
