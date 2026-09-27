## What this changes, and why

<!-- The why matters more than the what: the diff already says what. If a measurement
     prompted this, put the number here. If you rejected an alternative, say which. -->

## The checklist

- [ ] `cargo xtask ci` is green on my machine
- [ ] Every screen I changed has its snapshots reviewed with `cargo insta review`, at
      80 × 24 and at the large size, in each theme
- [ ] `docs/DESIGN.md` says what the screen, the key or the command now does
- [ ] The key table and the help overlay agree, which a test checks
- [ ] `CHANGELOG.md` says what changed
- [ ] A new runtime dependency, if there is one, is justified above with its measured
      effect on binary size and startup time. Development dependencies need no
      measurement.

## What I tested, and how

<!-- Not "it works". Which case, run how, against which SOLAR, with what result. A test
     that fails without your change and passes with it is the strongest thing you can
     write here. -->
