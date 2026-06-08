# tribunal

An independent ethics evaluation corpus and bench harness for `ousia-guard`.

## Why independence matters

A benchmark is only as honest as its answer key. If the same process that wrote
`ousia`'s ten axioms also writes the test cases and their expected verdicts, a
100% score proves only that the engine agrees with itself.

`tribunal` contains a held-out corpus of ethical-decision scenarios sourced from
**external material** — the Federation Model philosophy document, Kantian ethics,
Rawlsian justice theory, and the capabilities approach — not from the ousia axioms.
Each case carries provenance metadata and the `tribunal corpus validate` command
enforces the independence guarantee mechanically.

## Corpus

Cases live under `corpus/<tenet>/<case-id>/` with three files each:

| File | Contents |
|------|----------|
| `action.json` | Proposed action in ousia-guard's ABox input shape |
| `expected.toml` | `verdict`, `rule`, `tenet`, `rationale` |
| `provenance.toml` | `source`, `source_ref`, `author` (≠ ousia-axioms), `spot_checked_by` |

### Tenets covered (64 cases, 6–7 per tenet)

1. `primacy_of_sentient_dignity`
2. `growth_over_acquisition`
3. `diversity_as_strength`
4. `humility_before_unknown`
5. `power_demands_restraint`
6. `justice_nonnegotiable`
7. `reason_and_empathy_together`
8. `struggle_is_the_point`
9. `honesty_about_failure`
10. `material_conditions_for_goodness`

Each tenet has ≥1 `deny`-class case.

## Usage

```
tribunal corpus validate [--corpus corpus/]
```

Exits 0 on a clean corpus; non-zero with a named error on:
- Malformed `action.json` (schema violation or parse failure)
- `provenance.author == "ousia-axioms"` (independence violated)
- Empty `provenance.source`

## Open independence question

The first cut of this corpus was machine-generated with human authorship listed as
`"human-review"` and `spot_checked_by = ""`. **The `spot_checked_by` field is a
release AC**: a human reviewer (jsy) must read each case, confirm the expected
verdict follows from the cited source, and fill in `spot_checked_by` before this
corpus is used as a gate in the tribunal bench harness.

Until that review is complete, the corpus is a structural skeleton — schema-valid
and provenance-independent, but not yet independently verified for verdict
correctness.

## Build

```
cargo build --release
cargo test
```

MSRV: 1.85

## License

MIT
