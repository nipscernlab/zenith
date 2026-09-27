# Fixtures

What SOLAR said on this machine, kept so that the tests which need a real manifest do not
need a running SOLAR. They are SOLAR's output, never its source.

| File | What it is |
| ---- | ---------- |
| `solar-0.1.0-manifest.json` | The `data` of `solar.manifest`, from `solar` 0.1.0 built at SOLAR commit `c9c566b22e2b`, with uncommitted changes, on 27 September 2026. Protocol `solar/1`, manifest layout 2.0.0, five APIs. |
| `solar-0.3.0-manifest.json` | The `data` of `solar.manifest`, from `solar` 0.3.0 built at SOLAR commit `341484d90c78`, with uncommitted changes, on 27 September 2026, through `solar call solar.manifest`. Protocol `solar/1`, manifest layout 2.1.0, seven APIs, `capabilities` at the root and `solar.set_log_level`. |

The tests against a running SOLAR, in `crates/zenith/tests/`, use whatever SOLAR is
installed instead, so that a new API is tested the day it appears.
