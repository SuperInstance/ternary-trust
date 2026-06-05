#![forbid(unsafe_code)]

//! ternary-trust: Trust and relationship dynamics between agents.
//!
//! Models bidirectional trust scores, five trust stages (inspired by
//! dogmind-arena), trust decay over time, forgiveness mechanics, and a
//! trust network graph. Reputation scores aggregate trust from multiple
//! sources into a single metric.

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
    pub fn from_score(score: f64) -> Self {
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
#[derive(Debug, Clone, PartialEq)]
pub enum TrustEvent {
    /// A positive action that increases trust.
    Positive {
        from: String,
        to: String,
        magnitude: f64,
        description: String,
    },
    /// A negative action that decreases trust.
    Negative {
        from: String,
        to: String,
        magnitude: f64,
        description: String,
    },
    /// A betrayal — large negative trust impact.
    Betrayal {
        from: String,
        to: String,
        description: String,
    },
}

impl TrustEvent {
    /// Create a positive trust event.
    pub fn positive(from: impl Into<String>, to: impl Into<String>, magnitude: f64, desc: impl Into<String>) -> Self {
        TrustEvent::Positive {
            from: from.into(),
            to: to.into(),
            magnitude: magnitude.abs().min(1.0),
            description: desc.into(),
        }
    }

    /// Create a negative trust event.
    pub fn negative(from: impl Into<String>, to: impl Into<String>, magnitude: f64, desc: impl Into<String>) -> Self {
        TrustEvent::Negative {
            from: from.into(),
            to: to.into(),
            magnitude: magnitude.abs().min(1.0),
            description: desc.into(),
        }
    }

    /// Create a betrayal event (large negative impact, magnitude fixed at -0.5).
    pub fn betrayal(from: impl Into<String>, to: impl Into<String>, desc: impl Into<String>) -> Self {
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
    pub agent_a: String,
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
        self.relations.entry(key).or_insert_with(|| TrustRelation::new(a, b))
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
    pub agent: String,
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

    /// Compute the minimum (worst-case) reputation.
    pub fn min(&self) -> f64 {
        self.scores.iter().cloned().fold(0.0_f64, f64::min)
    }

    /// Compute the maximum (best-case) reputation.
    pub fn max(&self) -> f64 {
        self.scores.iter().cloned().fold(0.0_f64, f64::max)
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
    pub fn from_network(network: &TrustNetwork, agent: &str) -> Self {
        let mut rep = Self::new(agent);
        for rel in network.relations_for(agent) {
            if let Some(score) = rel.trust_from(agent, &rel.agent_a).or_else(|| rel.trust_from(agent, &rel.agent_b)) {
                // Get the score others have toward this agent
            }
            // Add the trust others have toward this agent
            if rel.agent_a == agent {
                rep.add(rel.b_to_a); // how B feels about A
            } else {
                rep.add(rel.a_to_b); // how A feels about this agent
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
        let mut net = TrustNetwork::with_config(
            TrustDecay::new(0.5, 0.0),
            ForgivenessConfig::none(),
        );
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
}
