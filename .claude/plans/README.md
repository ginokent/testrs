# `.claude/plans/`

Plan documents for substantial pieces of work — gap analyses,
implementation plans, design notes that informed multi-commit efforts.

Each file in this directory should be:

- **Self-contained.** Readable years later without the surrounding chat.
- **Time-stamped** in the filename or the top of the file.
- **Linked from BACKLOG.md** when a plan is in progress or queued.

## Conventions

- File names: `YYYY-MM-DD-short-topic.md`
- Use Markdown; no fancy formatting.
- Plans for shipped work stay here as a historical record (don't delete
  on completion). Add a `**Status: shipped in commit <sha>**` header at
  the top instead.

## Why this directory

The harness suggested `.claude/plans/` as the canonical location for
plan artefacts. Keeping them in-tree (rather than in chat-only
ephemera) means a fresh agent or a human reviewer can see the decisions
that drove past commits without needing to reconstruct them from
transcripts.

## Contents

The historical plans below document the major work that produced the
current state of the workspace:

- `2026-05-23-initial-design.md` — the initial library design that
  produced commit `793f067`.
- `2026-05-24-tier-b-additions.md` — the second gap analysis (after
  the initial release) and the items chosen for inclusion.
- `2026-05-24-tier-s-a-b-additions.md` — the third gap analysis,
  driving enum derive, state-machine framework, async support, etc.
- `2026-05-24-flat-map-and-field-attrs.md` — the fourth gap analysis,
  driving `#[arbitrary(strategy = ...)]`, `flat_map`, `prop_recursive!`,
  and float/char strategies.
