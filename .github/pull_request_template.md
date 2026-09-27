## What this changes, and why

<!-- The why matters more than the what: the diff already says what. If a measurement
     prompted this, put the number here. If you rejected an alternative, say which. -->

## The checklist

- [ ] `cargo xtask ci` is green on my machine
- [ ] Every screen I changed has its snapshots reviewed with `cargo insta review`, at
      80 × 24 and at the large size, in each theme
- [ ] `docs/DESIGN.md` says what the screen, the key or the command now does
- [ ] The key table and the help overlay agree, which a test checks
- [ ] `CHANGELOG.md` says what changed, in the same commit as the code, which CI checks
- [ ] Every other document the change affects moved with it, in the same commit: the
      README, the guides (`docs/TESTING_BY_HAND.md`, `docs/ADDING_A_FEATURE.md`,
      `CONTRIBUTING.md`, `AGENTS.md`) and `STATUS.md`. An outdated document is a defect
- [ ] If a screen the README shows changed, its pictures are redrawn with
      `cargo xtask screenshots`, which CI checks
- [ ] A row added to or changed in the table of `docs/TESTING_BY_HAND.md` has its step
      in `xtask/src/walkthrough.rs`, which CI runs
- [ ] A new runtime dependency, if there is one, is justified above with its measured
      effect on binary size and startup time. Development dependencies need no
      measurement.

## What I tested, and how

<!-- Not "it works". Which case, run how, against which SOLAR, with what result. A test
     that fails without your change and passes with it is the strongest thing you can
     write here. -->
