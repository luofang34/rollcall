#![allow(clippy::expect_used, clippy::panic)]

use crate::pull::ENDPOINTS;

/// The escrow must cover the networking surface, not just hosts: VLANs and
/// the VPN tunnel addresses (WireGuard peers, stored as IPAM IPs) are pulled
/// so a change to either shows up as a diff in `netbox/declared.json`.
#[test]
fn escrow_covers_vlans_and_ip_addresses() {
    assert!(ENDPOINTS.contains(&"ipam/vlans"), "VLANs are in the escrow");
    assert!(
        ENDPOINTS.contains(&"ipam/ip-addresses"),
        "IP addresses (incl. WireGuard peer addresses) are in the escrow"
    );
    assert!(ENDPOINTS.contains(&"ipam/prefixes"));
}

/// Credentials never land in git: no `users/*` or `secrets` endpoint is
/// pulled, and each endpoint appears once.
#[test]
fn escrow_excludes_credentials_and_has_no_duplicates() {
    for endpoint in ENDPOINTS {
        assert!(
            !endpoint.starts_with("users/") && !endpoint.contains("secret"),
            "escrow must not pull credential endpoint {endpoint}"
        );
    }
    let mut sorted = ENDPOINTS.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ENDPOINTS.len(), "no duplicate endpoints");
}
