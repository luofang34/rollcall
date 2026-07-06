# rollcall

Reconcile a declared fleet against observed reality.

A small set of Rust tools for one operator's on-prem estate. It takes the declared intent — device, network, and guest inventory, whether in TOML files or NetBox — and checks it against what the machines actually report: read-only SSH hardware sweeps, ICMP/HTTP service probes, and a committed git "escrow" of the NetBox database. It reports drift and renders a status PDF.

NetBox holds the roster; **rollcall** reads it and checks each name against reality — verifying every host is present and matches, keeping the NetBox export reproducible in git, and never writing to the fleet.

## Status

Experimental, built primarily for the author's own deployment. Contributions and audit are welcome; no promises about stability, API compatibility, or support.

## License

GNU AGPL-3.0-or-later. See LICENSE.
