# Clip retained formulas to verified stationary TUI edges

## Status

Proposed; reproduced with the reported formula and fixed header/footer layout.

## Context

The alternate-screen redraw fix in `1a1e8c42` retained a formula when its source
crossed the physical terminal edges. The user's running build was confirmed
as `2.3.2 (1a1e8c42)`, but a formula still became literal at the lower edge of
the Codex transcript. A Runtime API grid capture showed the opening bracket
and `W=\widehat S`, immediately followed by the fixed Back to bottom strip,
composer and status lines. These rows are inside the physical terminal; they
are not source edits and must never be included in formula coverage.

## Evidence

The stream-parser regression uses the same weight formula, including
`\frac{b_c(\mathbf r)\sigma(K)v_{\rm rel}}{\widehat q_{jc}}`, with a pinned
question and three fixed footer rows. The previous implementation failed the
footer clipping assertion. During repair another failure exposed a second
cause: after the closer was hidden, scanning recognized the inner
`(\mathbf r)` and replaced the verified complete formula with that fragment.

## Decision

Infer candidate content edges only from unchanged leading/trailing physical
rows that include nonblank text in two redraw snapshots. Admit those edges
only when the existing unique-content row displacement proof and all visible
formula spans agree. Preserve a fingerprint of the excluded edge rows; any
subsequent change invalidates that evidence instead of concealing new content.

Persist the admitted bounds with the formula and carry them into the overlay.
Keep the complete source geometry, but limit source coverage, cursor exclusion,
fallback color checks and pixel clipping to the admitted content rows. Clipped
formula spans do not overlap independent fixed-UI formulas. Once confirmed,
the grid carries the same boundary to later complete scans.

Skip rescanning verified cached fragments clipped at either content edge.
Both an orphan closer and an internal parenthesized expression otherwise can
replace the original formula. Ordinary source changes still invalidate the
cache and are scanned normally.

## Rejected alternatives

- Hardcode Codex footer text or row counts: translations, terminal dimensions
  and other applications do not share one layout.
- Ignore any source mismatch near the bottom: unrelated content would inherit
  a stale formula without displacement or boundary evidence.
- Limit only bitmap paint: coverage would still erase the fixed status/input
  text, and cursor filtering could discard the formula before paint.
- Treat a surviving inner formula as the full result: it loses mathematical
  meaning and moves the visible baseline.

## Consequences

No terminal cells, scrollback or copied source are changed. A constant-size
clip record is charged in the existing bounded formula cache. Stationary-edge
comparison is bounded by the two screen snapshots; fingerprint revalidation
visits only the excluded rows. This is a work bound, not measured UI latency.
This still requires a complete formula to have been observed and enough
unambiguous redraw evidence. It does not reconstruct never-seen source.

## Validation

The real terminal parser/adapter checks both content edges, repeated frames,
complete source identity, baseline translation, source coverage and a colored
footer. Negative cases invalidate clipped formulas when source text or fixed
UI rows change. Shared terminal-math and GPUI overlay tests pass (105 passed,
one pre-existing ignored test). The Runtime capture confirms the reported
input; automated geometry checks are not a live screenshot of the new build.

## Supersedes

Refines `2026-11-08-alternate-screen-formula-redraw.md`: physical viewport
bounds alone are insufficient for a TUI with fixed UI inside that viewport.

## Revisit when

A semantic TUI protocol supplies authoritative document rectangles and stable
formula identities in place of conservative screen-content matching.
