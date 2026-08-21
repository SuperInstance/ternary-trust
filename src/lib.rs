#![forbid(unsafe_code)]

//! ternary-trust: Trust and relationship dynamics between agents.
//!
//! Models bidirectional trust scores, five trust stages (inspired by
//! dogmind-arena), trust decay over time, forgiveness mechanics, and a
//! trust network graph. Reputation scores aggregate trust from multiple
//! sources into a single metric.
//!
//! # When to use this
//!
//! Reach for `ternary-trust` when one or more agents need to *remember* how
//! other agents have behaved over time and make decisions on that basis — for
//! example: multi-agent cooperation (prioritize allied partners, avoid hostile
//! ones), game AI relationships (NPCs remember the player's actions),
//! distributed-system reliability (circuit-break hostile dependencies), or
//! reputation-based partner routing. If a boolean "trusted / not trusted" is
//! too coarse but a full Bayesian reputation engine is too heavy, the
//! five-stage model here is a deliberate middle ground.
//!
//! Scores live on a `-1.0..=1.0` continuum and are clamped on every mutation,
//! so a single event can never push a score out of range.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Trust stages (inspired by dogmind-arena)
// ---------------------------------------------------------------------------

/// Five stages of trust, from hostile to alliance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TrustStage {
    Hostile,
    Wary,
    Neutral,
    Friendly,
    Allied,
}

impl TrustStage {
    /// Convert a numeric score (-1.0 to +1.0) into a trust stage.
    ///
    /// Boundaries: `< -0.6` → Hostile, `< -0.2` → Wary, `< 0.2` → Neutral,
    /// `< 0.6` → Friendly, otherwise Allied. A `NaN` score is reported as
    /// Neutral rather than silently classified via the catch-all arm.
    pub fn from_score(score: f64) -> Self {
        if score.is_nan() {
            return TrustStage::Neutral;
        }
        if score < -0.6 {
            TrustStage::Hostile
        } else if score < -0.2 {
            TrustStage::Wary
        } else if score < 0.2 {
            TrustStage::Neutral
        } else if score < 0.6 {
            TrustStage::Friendly
        } else {
            TrustStage::Allied
        }
    }

    /// Return the human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            TrustStage::Hostile => "hostile",
            TrustStage::Wary => "wary",
            TrustStage::Neutral => "neutral",
            TrustStage::Friendly => "friendly",
            TrustStage::Allied => "allied",
        }
    }
}

// ---------------------------------------------------------------------------
// TrustEvent
// ---------------------------------------------------------------------------

/// An action that modifies trust between two agents.
///
/// Direction convention: `from` is the agent whose trust is updated (the
/// observer/judge); `to` is the agent being judged. The event's delta is
/// applied to the `from → to` trust score. For example,
/// `Positive { from: "alice", to: "bob", .. }` raises Alice's trust in Bob
/// because Alice observed Bob doing something positive.
#[derive(Debug, Clone, PartialEq)]
pub enum TrustEvent {
    /// A positive action that increases trust.
    Positive {
        /// Agent whose trust is updated (the observer).
        from: String,
        /// Agent being judged (the observed).
        to: String,
        /// Magnitude of the trust increase (non-negative; capped to 1.0).
        magnitude: f64,
        /// Human-readable description of what was observed.
        description: String,
    },
    /// A negative action that decreases trust.
    Negative {
        /// Agent whose trust is updated (the observer).
        from: String,
        /// Agent being judged (the observed).
        to: String,
        /// Magnitude of the trust decrease (non-negative; capped to 1.0).
        magnitude: f64,
        /// Human-readable description of what was observed.
        description: String,
    },
    /// A betrayal — large negative trust impact (fixed -0.5 delta).
    Betrayal {
        /// Agent whose trust is updated (the observer).
        from: String,
        /// Agent being judged (the observed).
        to: String,
        /// Human-readable description of the betrayal.
        description: String,
    },
}

impl TrustEvent {
    /// Create a positive trust event.
    pub fn positive(
        from: impl Into<String>,
        to: impl Into<String>,
        magnitude: f64,
        desc: impl Into<String>,
    ) -> Self {
        TrustEvent::Positive {
            from: from.into(),
            to: to.into(),
            magnitude: magnitude.abs().min(1.0),
            description: desc.into(),
        }
    }

    /// Create a negative trust event.
    pub fn negative(
        from: impl Into<String>,
        to: impl Into<String>,
        magnitude: f64,
        desc: impl Into<String>,
    ) -> Self {
        TrustEvent::Negative {
            from: from.into(),
            to: to.into(),
            magnitude: magnitude.abs().min(1.0),
            description: desc.into(),
        }
    }

    /// Create a betrayal event (large negative impact, magnitude fixed at -0.5).
    pub fn betrayal(
        from: impl Into<String>,
        to: impl Into<String>,
        desc: impl Into<String>,
    ) -> Self {
        TrustEvent::Betrayal {
            from: from.into(),
            to: to.into(),
            description: desc.into(),
        }
    }

    /// Return the (from, to) pair.
    pub fn parties(&self) -> (&str, &str) {
        match self {
            TrustEvent::Positive { from, to, .. }
            | TrustEvent::Negative { from, to, .. }
            | TrustEvent::Betrayal { from, to, .. } => (from, to),
        }
    }

    /// Return the trust delta this event represents.
    pub fn delta(&self) -> f64 {
        match self {
            TrustEvent::Positive { magnitude, .. } => *magnitude,
            TrustEvent::Negative { magnitude, .. } => -*magnitude,
            TrustEvent::Betrayal { .. } => -0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// TrustDecay
// ---------------------------------------------------------------------------

/// Configuration for how trust fades over time.
#[derive(Debug, Clone, PartialEq)]
pub struct TrustDecay {
    /// Fraction of trust retained per tick (0.0 = instant decay, 1.0 = no decay).
    pub retention_rate: f64,
    /// Minimum absolute trust score (decays toward zero but not past this).
    pub floor: f64,
}

impl TrustDecay {
    /// Create a decay config.
    pub fn new(retention_rate: f64, floor: f64) -> Self {
        Self {
            retention_rate: retention_rate.clamp(0.0, 1.0),
            floor: floor.abs(),
        }
    }

    /// No decay — trust never fades.
    pub fn none() -> Self {
        Self {
            retention_rate: 1.0,
            floor: 0.0,
        }
    }

    /// Apply one tick of decay to a score, pulling it toward zero.
    pub fn apply(&self, score: f64) -> f64 {
        let decayed = score * self.retention_rate;
        if decayed.abs() < self.floor {
            if score > 0.0 {
                self.floor
            } else if score < 0.0 {
                -self.floor
            } else {
                0.0
            }
        } else {
            decayed
        }
    }
}

impl Default for TrustDecay {
    fn default() -> Self {
        Self::none()
    }
}

// ---------------------------------------------------------------------------
// ForgivenessConfig
// ---------------------------------------------------------------------------

/// How quickly negative trust recovers toward neutral.
#[derive(Debug, Clone, PartialEq)]
pub struct ForgivenessConfig {
    /// Amount of positive trust gained per forgiveness tick.
    pub recovery_rate: f64,
    /// Maximum negative trust that can be recovered per tick.
    pub max_recovery: f64,
}

impl ForgivenessConfig {
    /// Create a forgiveness config.
    pub fn new(recovery_rate: f64, max_recovery: f64) -> Self {
        Self {
            recovery_rate: recovery_rate.max(0.0),
            max_recovery: max_recovery.max(0.0),
        }
    }

    /// No forgiveness — negative trust persists.
    pub fn none() -> Self {
        Self {
            recovery_rate: 0.0,
            max_recovery: 0.0,
        }
    }

    /// Apply one tick of forgiveness to a score.
    pub fn apply(&self, score: f64) -> f64 {
        if score >= 0.0 {
            score
        } else {
            let recovery = self.recovery_rate.min(self.max_recovery).min(score.abs());
            score + recovery
        }
    }
}

impl Default for ForgivenessConfig {
    fn default() -> Self {
        Self::none()
    }
}

// ---------------------------------------------------------------------------
// TrustRelation
// ---------------------------------------------------------------------------

/// Bidirectional trust scores between two agents.
#[derive(Debug, Clone, PartialEq)]
pub struct TrustRelation {
    /// One endpoint of the relation.
    pub agent_a: String,
    /// The other endpoint of the relation.
    pub agent_b: String,
    /// Trust from A toward B (-1.0 to +1.0).
    pub a_to_b: f64,
    /// Trust from B toward A (-1.0 to +1.0).
    pub b_to_a: f64,
}

impl TrustRelation {
    /// Create a new relation starting at neutral (0.0) trust both ways.
    pub fn new(a: impl Into<String>, b: impl Into<String>) -> Self {
        Self {
            agent_a: a.into(),
            agent_b: b.into(),
            a_to_b: 0.0,
            b_to_a: 0.0,
        }
    }

    /// Create a relation with explicit initial scores.
    pub fn with_scores(
        a: impl Into<String>,
        b: impl Into<String>,
        a_to_b: f64,
        b_to_a: f64,
    ) -> Self {
        Self {
            agent_a: a.into(),
            agent_b: b.into(),
            a_to_b: a_to_b.clamp(-1.0, 1.0),
            b_to_a: b_to_a.clamp(-1.0, 1.0),
        }
    }

    /// Get trust from `from` toward `to`. Returns None if neither agent matches.
    pub fn trust_from(&self, from: &str, to: &str) -> Option<f64> {
        if from == self.agent_a && to == self.agent_b {
            Some(self.a_to_b)
        } else if from == self.agent_b && to == self.agent_a {
            Some(self.b_to_a)
        } else {
            None
        }
    }

    /// Apply a trust event delta to the correct direction.
    ///
    /// If the event's parties do not match this relation's two agents, the
    /// event is ignored (no-op) — a `TrustRelation` only tracks trust between
    /// its own `agent_a` and `agent_b`. Use [`TrustNetwork::apply_event`] to
    /// route an event to the correct relation automatically.
    pub fn apply_event(&mut self, event: &TrustEvent) {
        let (from, to) = event.parties();
        let delta = event.delta();
        if from == self.agent_a && to == self.agent_b {
            self.a_to_b = (self.a_to_b + delta).clamp(-1.0, 1.0);
        } else if from == self.agent_b && to == self.agent_a {
            self.b_to_a = (self.b_to_a + delta).clamp(-1.0, 1.0);
        }
    }

    /// Average trust in both directions.
    pub fn average(&self) -> f64 {
        (self.a_to_b + self.b_to_a) / 2.0
    }

    /// The trust stage for A→B.
    pub fn stage_a_to_b(&self) -> TrustStage {
        TrustStage::from_score(self.a_to_b)
    }

    /// The trust stage for B→A.
    pub fn stage_b_to_a(&self) -> TrustStage {
        TrustStage::from_score(self.b_to_a)
    }

    /// Does this relation involve the given agent?
    pub fn involves(&self, agent: &str) -> bool {
        self.agent_a == agent || self.agent_b == agent
    }
}

// ---------------------------------------------------------------------------
// TrustNetwork
// ---------------------------------------------------------------------------

/// A graph of trust relationships indexed by agent pair.
#[derive(Debug, Clone, Default)]
pub struct TrustNetwork {
    relations: HashMap<(String, String), TrustRelation>,
    decay: TrustDecay,
    forgiveness: ForgivenessConfig,
}

impl TrustNetwork {
    /// Create an empty network.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a network with decay and forgiveness configs.
    pub fn with_config(decay: TrustDecay, forgiveness: ForgivenessConfig) -> Self {
        Self {
            relations: HashMap::new(),
            decay,
            forgiveness,
        }
    }

    /// Get or create a relation between two agents (order-independent).
    fn key(a: &str, b: &str) -> (String, String) {
        if a <= b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        }
    }

    /// Get the relation between two agents, if any.
    pub fn get(&self, a: &str, b: &str) -> Option<&TrustRelation> {
        self.relations.get(&Self::key(a, b))
    }

    /// Get a mutable reference to the relation, creating it at neutral if needed.
    pub fn get_or_create(&mut self, a: &str, b: &str) -> &mut TrustRelation {
        let key = Self::key(a, b);
        self.relations
            .entry(key)
            .or_insert_with(|| TrustRelation::new(a, b))
    }

    /// Apply a trust event to the network.
    pub fn apply_event(&mut self, event: &TrustEvent) {
        let (from, to) = event.parties();
        let rel = self.get_or_create(from, to);
        rel.apply_event(event);
    }

    /// Apply one tick of decay and forgiveness to all relations.
    pub fn tick(&mut self) {
        for rel in self.relations.values_mut() {
            rel.a_to_b = self.decay.apply(rel.a_to_b);
            rel.b_to_a = self.decay.apply(rel.b_to_a);
            rel.a_to_b = self.forgiveness.apply(rel.a_to_b);
            rel.b_to_a = self.forgiveness.apply(rel.b_to_a);
        }
    }

    /// Number of trust relations in the network.
    pub fn relation_count(&self) -> usize {
        self.relations.len()
    }

    /// List all agents that have at least one relation.
    pub fn agents(&self) -> Vec<&str> {
        let mut set = std::collections::HashSet::new();
        for rel in self.relations.values() {
            set.insert(rel.agent_a.as_str());
            set.insert(rel.agent_b.as_str());
        }
        set.into_iter().collect()
    }

    /// Get all relations involving a given agent.
    pub fn relations_for(&self, agent: &str) -> Vec<&TrustRelation> {
        self.relations
            .values()
            .filter(|r| r.involves(agent))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ReputationScore
// ---------------------------------------------------------------------------

/// Aggregate trust score for an agent, computed from multiple sources.
#[derive(Debug, Clone, PartialEq)]
pub struct ReputationScore {
    /// The agent this reputation describes.
    pub agent: String,
    /// Trust scores that other agents hold toward this agent.
    pub scores: Vec<f64>,
}

impl ReputationScore {
    /// Create an empty reputation score.
    pub fn new(agent: impl Into<String>) -> Self {
        Self {
            agent: agent.into(),
            scores: Vec::new(),
        }
    }

    /// Add a trust score from one source.
    pub fn add(&mut self, score: f64) {
        self.scores.push(score.clamp(-1.0, 1.0));
    }

    /// Compute the average reputation. Returns 0.0 if no scores.
    pub fn average(&self) -> f64 {
        if self.scores.is_empty() {
            return 0.0;
        }
        self.scores.iter().sum::<f64>() / self.scores.len() as f64
    }

    /// Compute the minimum (worst-case) reputation. Returns 0.0 if no scores.
    ///
    /// Uses [`f64::total_cmp`] so a stray `NaN` is ordered consistently
    /// rather than panicking.
    pub fn min(&self) -> f64 {
        self.scores
            .iter()
            .copied()
            .min_by(f64::total_cmp)
            .unwrap_or(0.0)
    }

    /// Compute the maximum (best-case) reputation. Returns 0.0 if no scores.
    ///
    /// Uses [`f64::total_cmp`] so a stray `NaN` is ordered consistently
    /// rather than panicking.
    pub fn max(&self) -> f64 {
        self.scores
            .iter()
            .copied()
            .max_by(f64::total_cmp)
            .unwrap_or(0.0)
    }

    /// The trust stage for the average reputation.
    pub fn stage(&self) -> TrustStage {
        TrustStage::from_score(self.average())
    }

    /// Number of contributing scores.
    pub fn count(&self) -> usize {
        self.scores.len()
    }

    /// Build a reputation score from a network for a given agent.
    ///
    /// Collects the *inbound* trust toward `agent` — i.e. how every other
    /// agent that has a relation with `agent` feels about `agent`. `agent`'s
    /// own outbound opinions of others are not included. Returns an empty
    /// score (average 0.0, Neutral) if `agent` has no relations.
    pub fn from_network(network: &TrustNetwork, agent: &str) -> Self {
        let mut rep = Self::new(agent);
        for rel in network.relations_for(agent) {
            // Add the trust the OTHER agent holds toward `agent`.
            if rel.agent_a == agent {
                rep.add(rel.b_to_a); // agent_b's trust toward agent_a (== agent)
            } else if rel.agent_b == agent {
                rep.add(rel.a_to_b); // agent_a's trust toward agent_b (== agent)
            }
        }
        rep
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- TrustStage tests ---

    #[test]
    fn stage_from_score_boundaries() {
        assert_eq!(TrustStage::from_score(-1.0), TrustStage::Hostile);
        assert_eq!(TrustStage::from_score(-0.6), TrustStage::Wary);
        assert_eq!(TrustStage::from_score(-0.2), TrustStage::Neutral);
        assert_eq!(TrustStage::from_score(0.0), TrustStage::Neutral);
        assert_eq!(TrustStage::from_score(0.2), TrustStage::Friendly);
        assert_eq!(TrustStage::from_score(0.6), TrustStage::Allied);
        assert_eq!(TrustStage::from_score(1.0), TrustStage::Allied);
    }

    #[test]
    fn stage_labels() {
        assert_eq!(TrustStage::Hostile.label(), "hostile");
        assert_eq!(TrustStage::Allied.label(), "allied");
        assert_eq!(TrustStage::Neutral.label(), "neutral");
    }

    #[test]
    fn stage_ordering() {
        assert!(TrustStage::Hostile < TrustStage::Wary);
        assert!(TrustStage::Wary < TrustStage::Neutral);
        assert!(TrustStage::Neutral < TrustStage::Friendly);
        assert!(TrustStage::Friendly < TrustStage::Allied);
    }

    // --- TrustEvent tests ---

    #[test]
    fn positive_event_delta() {
        let e = TrustEvent::positive("a", "b", 0.3, "helped");
        assert_eq!(e.delta(), 0.3);
        assert_eq!(e.parties(), ("a", "b"));
    }

    #[test]
    fn negative_event_delta() {
        let e = TrustEvent::negative("a", "b", 0.2, "lied");
        assert_eq!(e.delta(), -0.2);
    }

    #[test]
    fn betrayal_delta() {
        let e = TrustEvent::betrayal("a", "b", "backstab");
        assert_eq!(e.delta(), -0.5);
    }

    #[test]
    fn event_magnitude_capped() {
        let e = TrustEvent::positive("a", "b", 5.0, "excessive");
        assert_eq!(e.delta(), 1.0);
    }

    // --- TrustDecay tests ---

    #[test]
    fn decay_reduces_score() {
        let d = TrustDecay::new(0.9, 0.0);
        let result = d.apply(0.5);
        assert!((result - 0.45).abs() < 1e-9);
    }

    #[test]
    fn decay_negative_score() {
        let d = TrustDecay::new(0.8, 0.0);
        let result = d.apply(-0.5);
        assert!((result - (-0.4)).abs() < 1e-9);
    }

    #[test]
    fn decay_floor() {
        let d = TrustDecay::new(0.5, 0.1);
        let result = d.apply(0.15);
        assert!((result - 0.1).abs() < 1e-9);
    }

    #[test]
    fn no_decay() {
        let d = TrustDecay::none();
        assert_eq!(d.apply(0.5), 0.5);
    }

    // --- ForgivenessConfig tests ---

    #[test]
    fn forgiveness_recover_negative() {
        let f = ForgivenessConfig::new(0.05, 0.2);
        let result = f.apply(-0.3);
        assert!((result - (-0.25)).abs() < 1e-9);
    }

    #[test]
    fn forgiveness_does_not_affect_positive() {
        let f = ForgivenessConfig::new(0.1, 0.5);
        assert_eq!(f.apply(0.5), 0.5);
    }

    #[test]
    fn no_forgiveness() {
        let f = ForgivenessConfig::none();
        assert_eq!(f.apply(-0.5), -0.5);
    }

    // --- TrustRelation tests ---

    #[test]
    fn relation_new_starts_neutral() {
        let r = TrustRelation::new("alice", "bob");
        assert_eq!(r.a_to_b, 0.0);
        assert_eq!(r.b_to_a, 0.0);
        assert_eq!(r.average(), 0.0);
        assert_eq!(r.stage_a_to_b(), TrustStage::Neutral);
    }

    #[test]
    fn relation_with_scores_clamped() {
        let r = TrustRelation::with_scores("a", "b", 2.0, -2.0);
        assert_eq!(r.a_to_b, 1.0);
        assert_eq!(r.b_to_a, -1.0);
    }

    #[test]
    fn relation_trust_from_lookup() {
        let r = TrustRelation::with_scores("alice", "bob", 0.5, -0.3);
        assert_eq!(r.trust_from("alice", "bob"), Some(0.5));
        assert_eq!(r.trust_from("bob", "alice"), Some(-0.3));
        assert_eq!(r.trust_from("alice", "carol"), None);
    }

    #[test]
    fn relation_apply_event() {
        let mut r = TrustRelation::new("a", "b");
        r.apply_event(&TrustEvent::positive("a", "b", 0.4, "helped"));
        assert!((r.a_to_b - 0.4).abs() < 1e-9);
        assert!((r.b_to_a).abs() < 1e-9); // unchanged
    }

    #[test]
    fn relation_involves() {
        let r = TrustRelation::new("x", "y");
        assert!(r.involves("x"));
        assert!(r.involves("y"));
        assert!(!r.involves("z"));
    }

    // --- TrustNetwork tests ---

    #[test]
    fn network_get_or_create() {
        let mut net = TrustNetwork::new();
        let rel = net.get_or_create("a", "b");
        assert_eq!(rel.agent_a, "a");
        assert_eq!(rel.agent_b, "b");
        assert_eq!(net.relation_count(), 1);
    }

    #[test]
    fn network_order_independent() {
        let mut net = TrustNetwork::new();
        net.get_or_create("b", "a");
        assert!(net.get("a", "b").is_some());
        assert!(net.get("b", "a").is_some());
        assert_eq!(net.relation_count(), 1);
    }

    #[test]
    fn network_apply_event() {
        let mut net = TrustNetwork::new();
        net.apply_event(&TrustEvent::positive("alice", "bob", 0.6, "cooperative"));
        let rel = net.get("alice", "bob").unwrap();
        assert!((rel.trust_from("alice", "bob").unwrap() - 0.6).abs() < 1e-9);
    }

    #[test]
    fn network_tick_decay() {
        let mut net =
            TrustNetwork::with_config(TrustDecay::new(0.5, 0.0), ForgivenessConfig::none());
        net.apply_event(&TrustEvent::positive("a", "b", 0.8, "good"));
        net.tick();
        let rel = net.get("a", "b").unwrap();
        assert!((rel.a_to_b - 0.4).abs() < 1e-9);
    }

    #[test]
    fn network_agents_list() {
        let mut net = TrustNetwork::new();
        net.apply_event(&TrustEvent::positive("a", "b", 0.1, "hi"));
        net.apply_event(&TrustEvent::positive("b", "c", 0.1, "hi"));
        let mut agents = net.agents();
        agents.sort();
        assert_eq!(agents, vec!["a", "b", "c"]);
    }

    #[test]
    fn network_relations_for_agent() {
        let mut net = TrustNetwork::new();
        net.apply_event(&TrustEvent::positive("a", "b", 0.1, "hi"));
        net.apply_event(&TrustEvent::positive("a", "c", 0.2, "hi"));
        assert_eq!(net.relations_for("a").len(), 2);
        assert_eq!(net.relations_for("b").len(), 1);
    }

    // --- ReputationScore tests ---

    #[test]
    fn reputation_empty_average() {
        let rep = ReputationScore::new("agent");
        assert_eq!(rep.average(), 0.0);
        assert_eq!(rep.count(), 0);
        assert_eq!(rep.stage(), TrustStage::Neutral);
    }

    #[test]
    fn reputation_average() {
        let mut rep = ReputationScore::new("agent");
        rep.add(0.5);
        rep.add(-0.3);
        assert!((rep.average() - 0.1).abs() < 1e-9);
    }

    #[test]
    fn reputation_min_max() {
        let mut rep = ReputationScore::new("agent");
        rep.add(0.8);
        rep.add(-0.4);
        assert!((rep.max() - 0.8).abs() < 1e-9);
        assert!((rep.min() - (-0.4)).abs() < 1e-9);
    }

    #[test]
    fn reputation_from_network() {
        let mut net = TrustNetwork::new();
        net.apply_event(&TrustEvent::positive("bob", "alice", 0.5, "helpful"));
        net.apply_event(&TrustEvent::negative("carol", "alice", 0.3, "rude"));
        let rep = ReputationScore::from_network(&net, "alice");
        assert_eq!(rep.count(), 2);
        // bob's trust toward alice = 0.5, carol's trust toward alice = -0.3
        assert!((rep.average() - 0.1).abs() < 1e-9);
    }

    // --- Regression tests for the fold-from-zero min/max bug ---

    #[test]
    fn reputation_max_all_negative() {
        // max of two negative scores must be the larger (closer to zero) one,
        // NOT 0.0 (which the old `fold(0.0, f64::max)` returned).
        let mut rep = ReputationScore::new("agent");
        rep.add(-0.3);
        rep.add(-0.5);
        assert!((rep.max() - (-0.3)).abs() < 1e-9, "max={}", rep.max());
        assert!((rep.min() - (-0.5)).abs() < 1e-9);
    }

    #[test]
    fn reputation_min_all_positive() {
        // min of two positive scores must be the smaller one, NOT 0.0.
        let mut rep = ReputationScore::new("agent");
        rep.add(0.5);
        rep.add(0.8);
        assert!((rep.min() - 0.5).abs() < 1e-9, "min={}", rep.min());
        assert!((rep.max() - 0.8).abs() < 1e-9);
    }

    #[test]
    fn reputation_min_max_empty() {
        let rep = ReputationScore::new("agent");
        assert_eq!(rep.min(), 0.0);
        assert_eq!(rep.max(), 0.0);
    }

    // --- Network/edge-case coverage ---

    #[test]
    fn reputation_from_network_empty() {
        // An agent with no relations has an empty (Neutral) reputation.
        let net = TrustNetwork::new();
        let rep = ReputationScore::from_network(&net, "ghost");
        assert_eq!(rep.count(), 0);
        assert_eq!(rep.average(), 0.0);
        assert_eq!(rep.stage(), TrustStage::Neutral);
    }

    #[test]
    fn reputation_from_network_inbound_only() {
        // Reputation collects how OTHERS feel about the target, not the
        // target's own outbound opinions. Here Alice rates Bob highly, but
        // that outbound opinion must not inflate Alice's own reputation —
        // only Bob's inbound opinion of Alice counts.
        let mut net = TrustNetwork::new();
        net.apply_event(&TrustEvent::positive("alice", "bob", 0.9, "great")); // outbound
        net.apply_event(&TrustEvent::positive("bob", "alice", 0.4, "ok")); // inbound
        let rep = ReputationScore::from_network(&net, "alice");
        assert_eq!(rep.count(), 1);
        assert!((rep.average() - 0.4).abs() < 1e-9);
    }

    #[test]
    fn stage_nan_is_neutral() {
        // A NaN must not silently fall through to Allied.
        assert_eq!(TrustStage::from_score(f64::NAN), TrustStage::Neutral);
        assert_eq!(TrustStage::from_score(f64::INFINITY), TrustStage::Allied);
        assert_eq!(
            TrustStage::from_score(f64::NEG_INFINITY),
            TrustStage::Hostile
        );
    }

    #[test]
    fn forgiveness_capped_at_zero() {
        // Forgiveness must never push a negative score past zero into
        // positive territory: the recovery is clamped by score.abs().
        let f = ForgivenessConfig::new(0.5, 1.0);
        assert_eq!(f.apply(-0.01), 0.0); // capped by score.abs()
        assert_eq!(f.apply(-0.4), 0.0); // recovery_rate > |score| -> lands on 0
        assert_eq!(f.apply(0.7), 0.7); // positive untouched

        // Capped by recovery_rate.
        let f = ForgivenessConfig::new(0.1, 1.0);
        assert!((f.apply(-0.4) - (-0.3)).abs() < 1e-9);
        // Capped by max_recovery.
        let f = ForgivenessConfig::new(1.0, 0.05);
        assert!((f.apply(-0.4) - (-0.35)).abs() < 1e-9);
    }

    #[test]
    fn decay_zero_stays_zero() {
        // A floor must not conjure trust out of a zero score.
        let d = TrustDecay::new(0.9, 0.2);
        assert_eq!(d.apply(0.0), 0.0);
    }

    #[test]
    fn relation_apply_event_ignores_unrelated_parties() {
        // A relation only tracks its own two agents; an event about a third
        // agent is a documented no-op and must not corrupt either score.
        let mut r = TrustRelation::with_scores("alice", "bob", 0.5, -0.2);
        r.apply_event(&TrustEvent::positive("alice", "carol", 0.9, "oops"));
        assert_eq!(r.a_to_b, 0.5);
        assert_eq!(r.b_to_a, -0.2);
    }

    // --- Manipulation resistance: scores are bounded and clamped ---

    #[test]
    fn trust_bounded_by_clamping() {
        // No amount of self-praise via repeated positive events can push a
        // score beyond +1.0 (or below -1.0). A single actor spamming events
        // saturates at the boundary rather than running away.
        let mut net = TrustNetwork::new();
        for _ in 0..100 {
            net.apply_event(&TrustEvent::positive(
                "mallory",
                "target",
                1.0,
                "self-praise",
            ));
        }
        let rel = net.get("mallory", "target").unwrap();
        assert_eq!(rel.trust_from("mallory", "target").unwrap(), 1.0);

        // And a flood of betrayals cannot go below -1.0.
        for _ in 0..100 {
            net.apply_event(&TrustEvent::betrayal("target", "mallory", "sabotage"));
        }
        let rel = net.get("mallory", "target").unwrap();
        assert_eq!(rel.trust_from("target", "mallory").unwrap(), -1.0);
    }

    #[test]
    fn single_event_delta_is_bounded() {
        // A single event changes trust by at most its magnitude (<=1.0), so
        // one interaction cannot arbitrarily inflate or crater a score.
        let mut net = TrustNetwork::new();
        net.apply_event(&TrustEvent::positive("a", "b", 0.3, "x"));
        let before = net.get("a", "b").unwrap().a_to_b;
        net.apply_event(&TrustEvent::negative("a", "b", 0.2, "y"));
        let after = net.get("a", "b").unwrap().a_to_b;
        let change = (after - before).abs();
        assert!(change <= 0.2 + 1e-9);
    }
}
