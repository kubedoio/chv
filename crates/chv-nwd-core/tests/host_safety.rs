//! Privileged host-safety regression test for the CHV firewall (#227).
//!
//! Proves on a real Linux host that CHV firewall policy is confined to
//! CHV-owned guest traffic:
//!
//! - unrelated host (container/CNI-style namespace-to-namespace) traffic keeps
//!   working after a CHV policy is applied (this fails against the old
//!   host-wide `policy drop` implementation);
//! - CHV-owned guest traffic is default-dropped with an empty policy;
//! - an allow rule restores guest connectivity;
//! - re-applying policy is idempotent and leaves unrelated traffic untouched.
//!
//! Requires root and `nft`/`ip`; skipped (trivially passes) otherwise. Run from
//! the CI-less local host with:
//!
//! ```text
//! cargo test -p chv-nwd-core --no-run
//! sudo -E $(find target/debug/deps -maxdepth 1 -name 'host_safety-*' -type f -executable |
//!     head -1) --ignored --exact host_safety::confines_policy_to_chv_owned_traffic
//! ```

use std::process::Command;

const UNREL_A: &str = "10.200.0.1";
const UNREL_B: &str = "10.200.0.2";
const GUEST_GW: &str = "10.201.0.1";
const GUEST_IP: &str = "10.201.0.2";

fn sh(args: &[&str]) -> Vec<u8> {
    let out = Command::new(args[0]).args(&args[1..]).output().unwrap();
    assert!(
        out.status.success(),
        "command failed ({status}): {args:?}\nstderr: {stderr}",
        status = out.status,
        stderr = String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

fn ns(ns: &str, args: &[&str]) -> Vec<u8> {
    let mut full = vec!["ip", "netns", "exec", ns, "ip"];
    full.extend_from_slice(args);
    sh(&full)
}

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim() == "0")
        .unwrap_or(false)
}

fn nft_available() -> bool {
    Command::new("nft")
        .arg("--version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Restores the host net.ipv4.ip_forward sysctl on drop.
struct IpForwardGuard(i32);

impl Drop for IpForwardGuard {
    fn drop(&mut self) {
        let _ = std::fs::write("/proc/sys/net/ipv4/ip_forward", self.0.to_string());
    }
}

/// Cleans up every resource the test created, best-effort.
struct Cleanup {
    namespaces: Vec<String>,
    links: Vec<String>,
    nft_table: String,
}

impl Cleanup {
    fn new(u: &str) -> Self {
        Cleanup {
            namespaces: vec![format!("us-a{u}"), format!("us-b{u}"), format!("gs{u}")],
            links: vec![
                format!("ua{u}"),
                format!("ub{u}"),
                format!("brhs{u}"),
                format!("gh{u}"),
                format!("gg{u}"),
            ],
            nft_table: format!("chvhs-{u}"),
        }
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _nft = Command::new("nft")
            .args(["delete", "table", "inet", &self.nft_table])
            .status();
        for ns_name in &self.namespaces {
            let _ = Command::new("ip").args(["netns", "del", ns_name]).status();
        }
        for link in &self.links {
            let _ = Command::new("ip").args(["link", "del", link]).status();
        }
    }
}

/// Ping from a namespace; returns whether at least one echo reply arrived.
fn ns_ping(ns_name: &str, dst: &str) -> bool {
    Command::new("ip")
        .args([
            "netns", "exec", ns_name, "ping", "-c", "1", "-W", "1", "-q", dst,
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
#[ignore = "requires root + real nftables on a Linux host; run locally via the fixture doc example"]
async fn confines_policy_to_chv_owned_traffic() {
    if !is_root() || !nft_available() {
        eprintln!("SKIP: host_safety needs root + nft; run via sudo outside CI");
        return;
    }

    let u = format!("{:05x}", std::process::id());

    // Build the "unrelated" host traffic path: two isolated network namespaces
    // bridged through the host forward path, none of whose interfaces CHV owns.
    sh(&["ip", "netns", "add", &format!("us-a{u}")]);
    sh(&["ip", "netns", "add", &format!("us-b{u}")]);
    sh(&[
        "ip",
        "link",
        "add",
        &format!("ua{u}"),
        "type",
        "veth",
        "peer",
        "name",
        &format!("ub{u}"),
    ]);
    sh(&[
        "ip",
        "link",
        "set",
        &format!("ua{u}"),
        "netns",
        &format!("us-a{u}"),
    ]);
    sh(&[
        "ip",
        "link",
        "set",
        &format!("ub{u}"),
        "netns",
        &format!("us-b{u}"),
    ]);
    ns(&format!("us-a{u}"), &["link", "set", "lo", "up"]);
    ns(&format!("us-b{u}"), &["link", "set", "lo", "up"]);
    ns(
        &format!("us-a{u}"),
        &[
            "addr",
            "add",
            &format!("{UNREL_A}/24"),
            "dev",
            &format!("ua{u}"),
        ],
    );
    ns(
        &format!("us-b{u}"),
        &[
            "addr",
            "add",
            &format!("{UNREL_B}/24"),
            "dev",
            &format!("ub{u}"),
        ],
    );
    ns(
        &format!("us-a{u}"),
        &["link", "set", &format!("ua{u}"), "up"],
    );
    ns(
        &format!("us-b{u}"),
        &["link", "set", &format!("ub{u}"), "up"],
    );

    // Build the "CHV guest" path: a CHV-owned bridge in the host netns plus a
    // guest namespace reached through a veth member enslaved to the bridge.
    sh(&["ip", "link", "add", &format!("brhs{u}"), "type", "bridge"]);
    sh(&[
        "ip",
        "addr",
        "add",
        &format!("{GUEST_GW}/24"),
        "dev",
        &format!("brhs{u}"),
    ]);
    sh(&["ip", "link", "set", &format!("brhs{u}"), "up"]);
    sh(&[
        "ip",
        "link",
        "add",
        &format!("gh{u}"),
        "type",
        "veth",
        "peer",
        "name",
        &format!("gg{u}"),
    ]);
    sh(&[
        "ip",
        "link",
        "set",
        &format!("gh{u}"),
        "master",
        &format!("brhs{u}"),
    ]);
    sh(&["ip", "link", "set", &format!("gh{u}"), "up"]);
    sh(&["ip", "netns", "add", &format!("gs{u}")]);
    sh(&[
        "ip",
        "link",
        "set",
        &format!("gg{u}"),
        "netns",
        &format!("gs{u}"),
    ]);
    ns(&format!("gs{u}"), &["link", "set", "lo", "up"]);
    ns(
        &format!("gs{u}"),
        &[
            "addr",
            "add",
            &format!("{GUEST_IP}/24"),
            "dev",
            &format!("gg{u}"),
        ],
    );
    ns(&format!("gs{u}"), &["link", "set", &format!("gg{u}"), "up"]);

    // The forward path between the two unrelated namespaces requires host
    // forwarding; remember and restore the sysctl later.
    let fwd_before: i32 = std::fs::read_to_string("/proc/sys/net/ipv4/ip_forward")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    let _fwd_guard = IpForwardGuard(fwd_before);
    if fwd_before != 1 {
        std::fs::write("/proc/sys/net/ipv4/ip_forward", "1").unwrap();
    }

    let cleanup = Cleanup::new(&u);

    // Baseline: both paths work before any CHV policy is applied.
    assert!(
        ns_ping(&format!("us-a{u}"), UNREL_B),
        "unrelated traffic must work at baseline"
    );
    assert!(
        ns_ping(&format!("gs{u}"), GUEST_GW),
        "guest->gateway must work at baseline"
    );

    // CHV-owned interface set: the topology bridge + its enslaved member.
    let owned = vec![format!("brhs{u}"), format!("gh{u}")];
    let table = format!("chvhs-{u}");

    // 1) EMPTY policy -> default-deny inside the boundary; unrelated unaffected.
    chv_nwd_core::firewall::apply_firewall_rules(&table, &owned, b"[]")
        .await
        .unwrap();
    if std::env::var("HOST_SAFETY_DUMP").is_ok() {
        let out = Command::new("nft")
            .args(["list", "table", "inet", &table])
            .output()
            .unwrap();
        eprintln!(
            "--- nft table dump ---\n{}",
            String::from_utf8_lossy(&out.stdout)
        );
    }
    assert!(
        ns_ping(&format!("us-a{u}"), UNREL_B),
        "REGRESSION (#227): unrelated host traffic must survive a CHV empty policy \
         (old code dropped it via host-wide policy drop)"
    );
    assert!(
        !ns_ping(&format!("gs{u}"), GUEST_GW),
        "guest->gateway must be default-DENIED inside the CHV boundary under an empty policy"
    );

    // 2) ALLOW policy -> guest connectivity restored; unrelated still untouched.
    let allow = br#"[{"direction":"inbound","protocol":"icmp","source_cidr":"10.201.0.0/24","action":"accept"}]"#;
    chv_nwd_core::firewall::apply_firewall_rules(&table, &owned, allow)
        .await
        .unwrap();
    assert!(
        ns_ping(&format!("gs{u}"), GUEST_GW),
        "guest->gateway must pass once an inbound allow rule matches"
    );
    assert!(
        ns_ping(&format!("us-a{u}"), UNREL_B),
        "unrelated host traffic must remain unaffected by CHV allow policy"
    );

    // 3) Re-apply EMPTY policy -> idempotent, guest default-deny re-established.
    chv_nwd_core::firewall::apply_firewall_rules(&table, &owned, b"[]")
        .await
        .unwrap();
    assert!(
        ns_ping(&format!("us-a{u}"), UNREL_B),
        "unrelated host traffic must remain unaffected after a re-apply"
    );
    assert!(
        !ns_ping(&format!("gs{u}"), GUEST_GW),
        "re-applied empty policy must re-establish CHV default-deny"
    );

    drop(cleanup);
}
