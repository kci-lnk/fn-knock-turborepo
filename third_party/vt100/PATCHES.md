# Local vt100 patches

This directory contains `vt100` 0.16.2 under its MIT license.

The local changes in `src/row.rs` fix the wide-character resize panic tracked
by [doy/vt100-rust#28](https://github.com/doy/vt100-rust/issues/28) and are
based on the defensive fixes from the still-unreleased upstream
[pull request #30](https://github.com/doy/vt100-rust/pull/30):

- shrinking a row uses `Row::truncate` so a split wide character is cleared;
- `Row::clear_wide` bounds-checks a missing continuation cell;
- `Row::erase` avoids subtraction underflow for a one-column orphaned cell.

Remove this vendored copy and return to the crates.io dependency once an
upstream release includes those fixes.
