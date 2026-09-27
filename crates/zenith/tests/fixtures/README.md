# Fixtures

| File | What it is |
| ---- | ---------- |
| `readme-session.json` | The session the README's screenshots show, recorded against `solar` 0.2.0 on 27 September 2026 by `against_solar::record_the_session_the_readme_shows`: every response line exactly as SOLAR wrote it, the round trip ZENITH measured for each, and SOLAR's standard error at `trace`. The one change made by hand is the user name in the paths `system.info` reports, which is `lab` here, so that no one's home directory is published. |

To record it again, against whatever SOLAR is installed:

```bash
ZENITH_RECORD_README=1 cargo nextest run --workspace -E 'test(record_the_session_the_readme_shows)'
```

Then replace the user name in the paths again, review the snapshots with
`cargo insta review`, and draw the pictures with `cargo xtask screenshots`.
