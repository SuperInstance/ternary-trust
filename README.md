# ternary-trust: Trust and relationship dynamics between agents

Models bidirectional trust scores between agents on a -1.0 to +1.0 scale, organized into five stages inspired by dogmind-arena: **Hostile**, **Wary**, **Neutral**, **Friendly**, **Allied**. Supports trust decay, forgiveness mechanics, and reputation aggregation across a network of relationships.

## Why This Exists

In multi-agent systems, agents need to remember how other agents have behaved. A simple boolean "trusted/not trusted" loses nuance — an agent that occasionally fails is different from one that actively betrays you. The five-stage model (from dogmind-arena) gives agents enough granularity to make nuanced decisions without overwhelming them with floating-point precision they can't act on.

## Core Concepts

- **TrustStage**: Five stages of trust — Hostile (< -0.6), Wary (-0.6 to -0.2), Neutral (-0.2 to 0.2), Friendly (0.2 to 0.6), Allied (> 0.6). Agents map numeric scores to stages for decision-making.
- **TrustRelation**: Bidirectional trust between two agents. Alice might trust Bob at 0.8 (Allied) while Bob trusts Alice at -0.3 (Wary).
- **TrustEvent**: An action that modifies trust. Positive (cooperation), Negative (lying), or Betrayal (large negative, fixed -0.5 delta).
- **TrustDecay**: Trust fades toward zero over time. Configured with a retention rate (fraction kept per tick) and a floor (minimum absolute trust).
- **ForgivenessConfig**: Negative trust slowly recovers toward zero. Separate from decay — decay pulls both directions; forgiveness only helps negative scores.
- **TrustNetwork**: A graph of `TrustRelation`s. Lookup by agent pair, apply events, tick decay/forgiveness across all relations.
- **ReputationScore**: Aggregate trust for one agent as seen by all others. Provides average, min, max, and stage.

## Quick Start

```toml
# Cargo.toml
[dependencies]
ternary-trust = "0.1"
```

```rust
use ternary_trust::*;

fn main() {
    let mut net = TrustNetwork::with_config(
        TrustDecay::new(0.95, 0.05),       // 5% decay per tick, floor at 0.05
        ForgivenessConfig::new(0.02, 0.1), // recover 0.02/tick, max 0.1/tick
    );

    // Alice helps Bob — trust increases
    net.apply_event(&TrustEvent::positive("alice", "bob", 0.3, "shared resources"));

    // Bob betrays Alice — big trust drop
    net.apply_event(&TrustEvent::betrayal("bob", "alice", "stole supplies"));

    let rel = net.get("alice", "bob").unwrap();
    println!("Alice→Bob: {:?} ({:.2})", rel.stage_a_to_b(), rel.a_to_b);
    println!("Bob→Alice: {:?} ({:.2})", rel.stage_b_to_a(), rel.b_to_a);

    // Check Alice's reputation across the network
    let rep = ReputationScore::from_network(&net, "alice");
    println!("Alice's reputation: {:.2} ({:?})", rep.average(), rep.stage());
}
```

## API Overview

| Type | Description |
|------|-------------|
| `TrustStage` | Five-stage enum: Hostile, Wary, Neutral, Friendly, Allied |
| `TrustRelation` | Bidirectional trust scores between two agents (-1.0 to +1.0) |
| `TrustEvent` | Action that modifies trust: Positive, Negative, or Betrayal |
| `TrustDecay` | Configures how trust fades toward zero per tick |
| `ForgivenessConfig` | Configures how negative trust recovers per tick |
| `TrustNetwork` | Graph of trust relationships with event application and tick |
| `ReputationScore` | Aggregate trust from multiple sources with stats |

## How It Works

Scores live on a -1.0 to +1.0 continuum, clamped on every mutation. `TrustStage::from_score()` maps ranges to one of five stages — agents use stages for decisions, not raw floats.

`TrustNetwork` stores relations in a `HashMap` keyed by a canonical (alphabetically-ordered) agent pair, so `get("alice", "bob")` and `get("bob", "alice")` return the same relation. Events specify a direction (from/to) so the correct directional score is updated.

The `tick()` method applies decay then forgiveness to every relation in the network. Decay multiplies scores by the retention rate, pulling toward zero. Forgiveness adds the recovery rate to negative scores, also pulling toward zero but only from below.

`ReputationScore::from_network()` collects how all other agents feel about a target agent, providing average, min, max, and stage for reputation-based decisions.

## Known Limitations

- **No persistence**: Trust state lives only in memory. You'll need your own serialization layer to persist across restarts.
- **Global tick, not real time**: Decay and forgiveness operate on abstract "ticks," not wall-clock time. You decide what a tick means.
- **No transitive trust**: If A trusts B and B trusts C, there's no built-in inference for A→C trust. That's a different algorithm (e.g., PageRank-style).
- **Symmetric key, asymmetric scores**: The network canonicalizes agent pair ordering for storage, but the trust scores are still directional. This can be confusing if you expect `get("b", "a")` to return `b_to_a` directly.
- **No trust justification storage**: Events modify scores but aren't stored in the network. You'd need a separate event history (see `ternary-event`).
- **Linear network scans**: `relations_for()` and `agents()` iterate all relations. Fine for hundreds; not optimized for millions.

## Use Cases

- **Multi-agent cooperation**: Agents track who has helped or hindered them. Allied agents get priority access; hostile agents get avoided.
- **Game AI relationships**: NPCs remember player actions. Betrayal drops trust dramatically; consistent help builds alliance.
- **Distributed system reliability**: Services track which dependencies have failed them. Wary services add retries; hostile services circuit-break.
- **Reputation-based routing**: Network chooses partners based on reputation scores aggregated from past interactions.

## Ecosystem Context

Part of the **SuperInstance** ternary crate family. Relates to:

- **ternary-event**: Trust events (betrayal, cooperation) can be published to the event bus for system-wide notification
- **ternary-command**: Failed commands may trigger negative trust events; successful ones build trust
- **ternary-kalman**: Trust scores could be estimated state rather than direct measurement

This crate is a leaf dependency — it doesn't depend on other ternary crates.

## License

MIT

## See Also
- **ternary-consensus** — related
- **ternary-quorum** — related
- **ternary-room** — related
- **ternary-signaling** — related
- **ternary-steward** — related

