# ternary-trust

Trust and relationship dynamics for ternary agents — pure Rust, zero dependencies.

Inspired by [dogmind-arena](https://github.com/SuperInstance/dogmind-arena)'s 5 trust stages and findings from the trust-genome experiment: **genomes alone don't produce trust — agents need learning and forgiveness mechanics.** Forgiveness rates of 0.5–0.7 produce the most bonding behavior.

## Features

- **`TrustStage`** — 5-stage enum: Stranger → Wary → Acquaintance → Friend → Companion
- **`TrustRelation`** — Bidirectional trust scores with update, decay, and query methods
- **`TrustEvent`** — Actions that affect trust (Cooperate, Defect, Ignore, Forgive, Betray) with ternary impact
- **`TrustDecay`** — Configurable trust fading over time with `step()` method
- **`ForgivenessConfig`** — Negative trust recovery (rate, threshold, cooldown)
- **`TrustNetwork`** — Graph of trust relationships between N agents with query methods
- **`ReputationScore`** — Aggregate trust from multiple sources (weighted avg, min/max, consensus)

## Quick Start

```rust
use ternary_trust::*;

// Create a network of agents
let mut net = TrustNetwork::with_agents(&["alice", "bob", "carol"]);

// Alice cooperates with Bob
net.apply_event("alice", "bob", TrustEvent::Cooperate(20));

// Bob cooperates back
net.apply_event("bob", "alice", TrustEvent::Cooperate(15));

// Check mutual trust
assert_eq!(net.mutual_trust("alice", "bob"), 17.5);

// Trust stages
let rel = net.trust_between("alice", "bob").unwrap();
assert_eq!(rel.stage_a_to_b(), TrustStage::Friend);     // 20
assert_eq!(rel.stage_b_to_a(), TrustStage::Friend);     // 15

// Forgiveness (inspired by trust-genome findings)
let cfg = ForgivenessConfig::recommended(); // rate 0.6
let (new_trust, _) = cfg.apply(-25, 10);
assert!(new_trust > -25);
```

## Trust Stages

| Stage         | Range      | Description                |
|---------------|------------|----------------------------|
| Stranger      | < -20      | Hostile or unknown         |
| Wary          | -20 … 0    | Cautious, not yet neutral  |
| Acquaintance  |  0 … 20    | Known but not close        |
| Friend        | 20 … 50    | Positive regard            |
| Companion     | > 50       | Deep bond                  |

## Forgiveness

The trust-genome experiment showed that forgiveness rates between **0.5 and 0.7** produce the most bonding. Too low and agents never recover from mistakes; too high and trust becomes meaningless.

```rust
let cfg = ForgivenessConfig {
    rate: 0.6,        // move 60% toward zero per forgiveness step
    threshold: -50,   // only forgive above this floor
    cooldown: 5,      // minimum steps between forgiveness
};
```

## Known Limitations

- **Trust scores are global integers (i32)** — no per-context or per-domain trust tracking
- **No persistence** — trust state exists only in memory; serialize externally if needed
- **No concurrency safety** — `TrustNetwork` is not `Send + Sync`; wrap in a `Mutex` for shared access
- **Canonical key ordering** — agent names are compared lexicographically; very large name sets may have minor overhead
- **Linear decay only** — trust decays by a fixed fraction per step; no exponential or custom decay curves
- **No trust propagation** — if A trusts B and B trusts C, that doesn't affect A→C trust (no transitive inference)
- **Reputation is a simple average** — no Bayesian, exponential moving average, or time-weighted reputation
- **Forgiveness applies uniformly** — no per-agent forgiveness history or graduated recovery
- **Trust events carry static impact** — no contextual modifiers (e.g., repeated betrayal escalating impact)

## License

MIT
