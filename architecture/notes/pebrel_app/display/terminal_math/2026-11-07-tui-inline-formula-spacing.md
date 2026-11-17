# Compact inline formulas inside fixed-grid text runs

## Status

Proposed; Windows regression tests verify projection and hit coordinates.

## Context

Terminal math draws a compact formula over its longer delimited source. The
alternate-screen path previously disabled all horizontal projection to protect
TUI columns, sidebars and background panels. This left the source delimiters'
width as a visible gap between a formula and following prose, including table
cells emitted by Codex after its Unicode math conversion was disabled.

## Evidence

The captured terminal grid contains `\(a\)，...` without spaces before the
comma. A regression using the terminal stream parser reproduces the comma at
pixel 88 while the formula box ends at pixel 64 with eight-pixel cells.
`R_0` independently hit the uppercase-shell-variable rejection rule.

## Decision

Keep `LineProjection` authoritative for text, images, formula origins and
inverse selection coordinates. In alternate screens, compact inline source
only within the following text run. Two consecutive spaces, vertical borders,
image cells and non-default ANSI backgrounds terminate that run. Columns at
and beyond the boundary retain their original positions. Expansion beyond the
source width does not join this bounded compaction path. Normal shell rows
retain their existing unrestricted projection.

Derive each boundary from the current scan; it is not persisted across grid
updates. Reset accumulated shifts at boundaries, including before later
formulas. Newly empty padding maps back to its source boundary for selection.
Recognize single uppercase variables with numeric subscripts while continuing
to reject multi-letter shell identifiers.

## Rejected alternatives

- Enable unrestricted reflow in alternate screens: it shifts neighboring table
  columns, ANSI surfaces and image placements.
- Move only the painted suffix: cursor, links, backgrounds and selection would
  disagree with the visible text.
- Right-align the formula inside its original source: it moves the excess gap
  ahead of the symbol instead of removing it from the text run.

## Consequences

The grid, copied source and terminal input remain unchanged. Intentional runs
of two or more spaces are treated as fixed-grid alignment boundaries. There is
one additional end column per projection span; no per-cell projection table
or extra rendering resource is retained. Boundary discovery scans at most the
remaining columns of each single-row formula, plus the existing resolved-cell
list to stop at ANSI backgrounds. This is a bound, not a timing guarantee.

## Validation

The real terminal-parser regression checks Chinese wide cells, uppercase
subscripts, punctuation placement, fixed table columns and inverse hits.
Additional cases cover multiple formulas, padding, borders and ANSI surfaces.
Existing formula/image, selection and scrolling regressions remain applicable.
These checks validate layout plans; they do not constitute a live-window
screenshot of the changed executable.

## Supersedes

None; refines the alternate-screen projection boundary while keeping the
shared projection ownership.

## Revisit when

Terminal output supplies semantic table cells or native document spans that
can replace the current fixed-grid boundary evidence.
