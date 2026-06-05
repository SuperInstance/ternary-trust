//! # ternary-trust
//!
//! Trust and relationship dynamics for ternary agents.
//!
//! Inspired by dogmind-arena's 5 trust stages and findings from the trust-genome experiment:
//! genomes alone don't produce trust — agents need learning and forgiveness mechanics.
//! Forgiveness rates of 0.5–0.7 produce the most bonding behavior.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Trust thresholds & bounds
// ---------------------------------------------------------------------------

/// Minimum possible trust score.
pub const TRUST_MIN: i32 = -100;
/// Maximum possible trust score.
pub const TRUST_MAX: i32 = 100;

/// Default trust decay rate per step (applied toward zero).
pub const DEFAULT_DECAY_RATE: f64 = 0.02;

// ---------------------------------------------------------------------------
// TrustStage — five stages from dogmind-arena
// ---------------------------------------------------------------------------

/// Trust stages inspired by dogmind-arena.
///
/// | Stage         | Trust range |
/// |---------------|-------------|
/// | Stranger      | < -20       |
/// | Wary          | -20 … 0     |
/// | Acquaintance  |  0 … 20     |
/// | Friend        | 20 … 50     |
/// | Companion     | > 50        |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrustStage {
    /// Hostile or unknown — trust below -20.
    Stranger,
    /// Cautious — trust between -20 and 0.
    Wary,
    /// Known but not close — trust between 0 and 20.
    Acquaintance,
    /// Positive regard — trust between 20 and 50.
    Friend,
    /// Deep bond — trust above 50.
    Companion,
}

impl TrustStage {
    /// Determine the trust stage from a raw trust score.
    pub fn from_trust(score: i32) -> Self {
        if score < -20 {
            TrustStage::Stranger
        } else if score < 0 {
            TrustStage::Wary
        } else if score < 20 {
            TrustStage::Acquaintance
        } else if score < 50 {
            TrustStage::Friend
        } else {
            TrustStage::Companion
        }
    }

    /// Human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            TrustStage::Stranger => "Stranger",
            TrustStage::Wary => "Wary",
            TrustStage::Acquaintance => "Acquaintance",
            TrustStage::Friend => "Friend",
            TrustStage::Companion => "Companion",
        }
    }

    /// Ordinal value (0 = Stranger … 4 = Companion).
    pub fn ordinal(&self) -> u8 {
        match self {
            TrustStage::Stranger => 0,
            TrustStage::Wary => 1,
            TrustStage::Acquaintance => 2,
            TrustStage::Friend => 3,
            TrustStage::Companion => 4,
        }
    }
}

impl fmt::Display for TrustStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

// ---------------------------------------------------------------------------
// TrustEvent — actions that affect trust
// ---------------------------------------------------------------------------

/// Actions that affect trust, each with a ternary impact magnitude.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustEvent {
    /// Positive cooperation. Default impact: +10.
    Cooperate(i32),
    /// Defection / non-cooperation. Default impact: -15.
    Defect(i32),
    /// Ignoring an agent. Default impact: -5.
    Ignore(i32),
    /// Forgiveness gesture. Default impact: +8.
    Forgive(i32),
    /// Serious betrayal. Default impact: -30.
    Betray(i32),
}

impl TrustEvent {
    /// Create with default ternary impact.
    pub fn default_impact(&self) -> i32 {
        match self {
            TrustEvent::Cooperate(_) => 10,
            TrustEvent::Defect(_) => -15,
            TrustEvent::Ignore(_) => -5,
            TrustEvent::Forgive(_) => 8,
            TrustEvent::Betray(_) => -30,
        }
    }

    /// Get the effective impact value.
    pub fn impact(&self) -> i32 {
        match self {
            TrustEvent::Cooperate(v)
            | TrustEvent::Defect(v)
            | TrustEvent::Ignore(v)
            | TrustEvent::Forgive(v)
            | TrustEvent::Betray(v) => *v,
        }
    }

    /// Is this a positive (trust-increasing) event?
    pub fn is_positive(&self) -> bool {
        self.impact() > 0
    }

    /// Is this a negative (trust-decreasing) event?
    pub fn is_negative(&self) -> bool {
        self.impact() < 0
    }
}

impl Default for TrustEvent {
    fn default() -> Self {
        TrustEvent::Cooperate(10)
    }
}

// ---------------------------------------------------------------------------
// TrustRelation — bidirectional trust between two agents
// ---------------------------------------------------------------------------

/// Bidirectional trust scores between two agents (A and B).
#[derive(Debug, Clone, PartialEq)]
pub struct TrustRelation {
    /// How much A trusts B.
    pub a_trusts_b: i32,
    /// How much B trusts A.
    pub b_trusts_a: i32,
}

impl TrustRelation {
    /// Create a new relation starting at neutral trust (0, 0).
    pub fn new() -> Self {
        Self {
            a_trusts_b: 0,
            b_trusts_a: 0,
        }
    }

    /// Create from explicit scores.
    pub fn from_scores(a: i32, b: i32) -> Self {
        Self {
            a_trusts_b: clamp(a),
            b_trusts_a: clamp(b),
        }
    }

    /// Apply an event from A toward B (affects a_trusts_b).
    pub fn apply_event_a_to_b(&mut self, event: TrustEvent) {
        self.a_trusts_b = clamp(self.a_trusts_b + event.impact());
    }

    /// Apply an event from B toward A (affects b_trusts_a).
    pub fn apply_event_b_to_a(&mut self, event: TrustEvent) {
        self.b_trusts_a = clamp(self.b_trusts_a + event.impact());
    }

    /// Average of both directions.
    pub fn average(&self) -> f64 {
        (self.a_trusts_b as f64 + self.b_trusts_a as f64) / 2.0
    }

    /// Trust stage of A toward B.
    pub fn stage_a_to_b(&self) -> TrustStage {
        TrustStage::from_trust(self.a_trusts_b)
    }

    /// Trust stage of B toward A.
    pub fn stage_b_to_a(&self) -> TrustStage {
        TrustStage::from_trust(self.b_trusts_a)
    }

    /// Are both directions at least Acquaintance?
    pub fn is_mutual_acquaintance(&self) -> bool {
        self.stage_a_to_b().ordinal() >= TrustStage::Acquaintance.ordinal()
            && self.stage_b_to_a().ordinal() >= TrustStage::Acquaintance.ordinal()
    }

    /// Apply symmetric decay toward zero for both directions.
    pub fn decay(&mut self, rate: f64) {
        self.a_trusts_b = decay_toward_zero(self.a_trusts_b, rate);
        self.b_trusts_a = decay_toward_zero(self.b_trusts_a, rate);
    }
}

impl Default for TrustRelation {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TrustDecay — configurable trust fading
// ---------------------------------------------------------------------------

/// Trust decays toward zero over time at a configurable rate.
#[derive(Debug, Clone)]
pub struct TrustDecay {
    /// Current trust value.
    pub trust: i32,
    /// Decay rate per step (0.0–1.0).
    pub decay_rate: f64,
}

impl TrustDecay {
    /// Create with a specific decay rate.
    pub fn new(trust: i32, decay_rate: f64) -> Self {
        Self {
            trust: clamp(trust),
            decay_rate: decay_rate.clamp(0.0, 1.0),
        }
    }

    /// Create with default decay rate.
    pub fn with_default_rate(trust: i32) -> Self {
        Self::new(trust, DEFAULT_DECAY_RATE)
    }

    /// Advance one time step — trust moves toward zero by decay_rate fraction.
    pub fn step(&mut self) {
        self.trust = decay_toward_zero(self.trust, self.decay_rate);
    }

    /// Run N decay steps.
    pub fn step_n(&mut self, n: u32) {
        for _ in 0..n {
            self.step();
        }
    }

    /// Current trust stage.
    pub fn stage(&self) -> TrustStage {
        TrustStage::from_trust(self.trust)
    }
}

// ---------------------------------------------------------------------------
// ForgivenessConfig — negative trust recovery
// ---------------------------------------------------------------------------

/// Configuration for how quickly negative trust recovers.
///
/// Inspired by the trust-genome finding that forgiveness rates of 0.5–0.7
/// produce the most bonding between agents.
#[derive(Debug, Clone)]
pub struct ForgivenessConfig {
    /// Rate of forgiveness per step (0.0–1.0). Recommended: 0.5–0.7.
    pub rate: f64,
    /// Trust must be at or above this threshold for forgiveness to kick in
    /// (i.e. only applies when trust is negative and above this floor).
    pub threshold: i32,
    /// Minimum steps between forgiveness applications.
    pub cooldown: u32,
}

impl ForgivenessConfig {
    /// Create with recommended defaults (rate 0.6, threshold -50, cooldown 5).
    pub fn recommended() -> Self {
        Self {
            rate: 0.6,
            threshold: -50,
            cooldown: 5,
        }
    }

    /// Apply one forgiveness step to a trust value.
    ///
    /// If trust is negative and above `threshold`, move it toward zero by `rate`.
    /// Returns `(new_trust, steps_since_last_forgiveness)`.
    pub fn apply(&self, trust: i32, steps_since_last: u32) -> (i32, u32) {
        if trust >= 0 || trust < self.threshold || steps_since_last < self.cooldown {
            return (trust, steps_since_last + 1);
        }
        let new_trust = decay_toward_zero(trust, self.rate);
        (new_trust, 0)
    }
}

impl Default for ForgivenessConfig {
    fn default() -> Self {
        Self::recommended()
    }
}

// ---------------------------------------------------------------------------
// TrustNetwork — graph of trust relationships
// ---------------------------------------------------------------------------

/// A graph of trust relationships between N named agents.
#[derive(Debug, Clone)]
pub struct TrustNetwork {
    relations: HashMap<(String, String), TrustRelation>,
    agents: Vec<String>,
}

impl TrustNetwork {
    /// Create an empty network.
    pub fn new() -> Self {
        Self {
            relations: HashMap::new(),
            agents: Vec::new(),
        }
    }

    /// Create a network with N named agents (all starting at neutral trust).
    pub fn with_agents(names: &[&str]) -> Self {
        let mut net = Self::new();
        for &name in names {
            net.add_agent(name);
        }
        net
    }

    /// Register an agent in the network.
    pub fn add_agent(&mut self, name: &str) {
        let name = name.to_string();
        if !self.agents.contains(&name) {
            for other in &self.agents {
                self.relations
                    .insert(canonical_key(&name, other), TrustRelation::new());
            }
            self.agents.push(name);
        }
    }

    /// Number of agents in the network.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Get the trust relation between two agents.
    pub fn trust_between(&self, a: &str, b: &str) -> Option<&TrustRelation> {
        self.relations.get(&canonical_key(a, b))
    }

    /// Get mutable trust relation between two agents.
    pub fn trust_between_mut(&mut self, a: &str, b: &str) -> Option<&mut TrustRelation> {
        self.relations.get_mut(&canonical_key(a, b))
    }

    /// Average trust of all agents toward `agent` (how much others trust them).
    pub fn reputation_of(&self, agent: &str) -> f64 {
        let mut sum = 0i64;
        let mut count = 0u64;
        for ((a, b), rel) in &self.relations {
            if b == agent {
                // canonical key (a,b) where a < b. a_trusts_b = how much a trusts b = agent.
                sum += rel.a_trusts_b as i64;
                count += 1;
            } else if a == agent {
                // b_trusts_a = how much b trusts a = agent.
                sum += rel.b_trusts_a as i64;
                count += 1;
            }
        }
        if count == 0 {
            0.0
        } else {
            sum as f64 / count as f64
        }
    }

    /// Agent with highest average trust from others.
    pub fn most_trusted(&self) -> Option<&str> {
        self.agents
            .iter()
            .max_by(|a, b| {
                self.reputation_of(a)
                    .partial_cmp(&self.reputation_of(b))
                    .unwrap()
            })
            .map(|s| s.as_str())
    }

    /// Agent with lowest average trust from others.
    pub fn least_trusted(&self) -> Option<&str> {
        self.agents
            .iter()
            .min_by(|a, b| {
                self.reputation_of(a)
                    .partial_cmp(&self.reputation_of(b))
                    .unwrap()
            })
            .map(|s| s.as_str())
    }

    /// Mutual trust score between two agents (average of both directions).
    pub fn mutual_trust(&self, a: &str, b: &str) -> f64 {
        self.trust_between(a, b).map_or(0.0, |r| r.average())
    }

    /// Trust circle: all agents whose mutual trust with `agent` is >= `min_trust`.
    pub fn trust_circle(&self, agent: &str, min_trust: f64) -> Vec<&str> {
        self.agents
            .iter()
            .filter(|other| *other != agent && self.mutual_trust(agent, other) >= min_trust)
            .map(|s| s.as_str())
            .collect()
    }

    /// Apply an event from `source` toward `target`.
    pub fn apply_event(&mut self, source: &str, target: &str, event: TrustEvent) {
        let ck = canonical_key(source, target);
        if let Some(rel) = self.relations.get_mut(&ck) {
            if ck == (source.to_string(), target.to_string()) {
                rel.apply_event_a_to_b(event);
            } else {
                // canonical key is (target, source), so source is "b" in the relation
                rel.apply_event_b_to_a(event);
            }
        }
    }

    /// Apply decay to all relations.
    pub fn decay_all(&mut self, rate: f64) {
        for rel in self.relations.values_mut() {
            rel.decay(rate);
        }
    }
}

impl Default for TrustNetwork {
    fn default() -> Self {
        Self::new()
    }
}

fn canonical_key(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

// ---------------------------------------------------------------------------
// ReputationScore — aggregate trust from multiple sources
// ---------------------------------------------------------------------------

/// Methods for aggregating trust scores from multiple sources.
#[derive(Debug, Clone)]
pub struct ReputationScore {
    /// Individual trust scores from different sources.
    pub scores: Vec<i32>,
}

impl ReputationScore {
    /// Create from a slice of scores.
    pub fn new(scores: &[i32]) -> Self {
        Self {
            scores: scores.iter().map(|&s| clamp(s)).collect(),
        }
    }

    /// Simple (unweighted) average.
    pub fn average(&self) -> f64 {
        if self.scores.is_empty() {
            return 0.0;
        }
        self.scores.iter().map(|&s| s as f64).sum::<f64>() / self.scores.len() as f64
    }

    /// Weighted average. Panics if weights.len() != scores.len().
    pub fn weighted_average(&self, weights: &[f64]) -> f64 {
        assert_eq!(weights.len(), self.scores.len(), "weights must match scores");
        if self.scores.is_empty() {
            return 0.0;
        }
        let total_weight: f64 = weights.iter().sum();
        if total_weight == 0.0 {
            return 0.0;
        }
        let sum: f64 = self
            .scores
            .iter()
            .zip(weights.iter())
            .map(|(&s, &w)| s as f64 * w)
            .sum();
        sum / total_weight
    }

    /// Minimum score (most pessimistic view).
    pub fn min(&self) -> i32 {
        self.scores.iter().copied().min().unwrap_or(0)
    }

    /// Maximum score (most optimistic view).
    pub fn max(&self) -> i32 {
        self.scores.iter().copied().max().unwrap_or(0)
    }

    /// Consensus: fraction of scores that are non-negative (>= 0).
    pub fn consensus(&self) -> f64 {
        if self.scores.is_empty() {
            return 0.0;
        }
        let positive = self.scores.iter().filter(|&&s| s >= 0).count();
        positive as f64 / self.scores.len() as f64
    }

    /// Overall trust stage based on average.
    pub fn stage(&self) -> TrustStage {
        TrustStage::from_trust(self.average().round() as i32)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn clamp(v: i32) -> i32 {
    v.clamp(TRUST_MIN, TRUST_MAX)
}

fn decay_toward_zero(v: i32, rate: f64) -> i32 {
    if v == 0 {
        return 0;
    }
    let delta = (v.abs() as f64 * rate).round() as i32;
    if v > 0 {
        (v - delta).max(0)
    } else {
        (v + delta).min(0)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- TrustStage ---------------------------------------------------------

    #[test]
    fn test_trust_stage_stranger() {
        assert_eq!(TrustStage::from_trust(-50), TrustStage::Stranger);
        assert_eq!(TrustStage::from_trust(-100), TrustStage::Stranger);
        assert_eq!(TrustStage::from_trust(-21), TrustStage::Stranger);
    }

    #[test]
    fn test_trust_stage_wary() {
        assert_eq!(TrustStage::from_trust(-20), TrustStage::Wary);
        assert_eq!(TrustStage::from_trust(-1), TrustStage::Wary);
    }

    #[test]
    fn test_trust_stage_acquaintance() {
        assert_eq!(TrustStage::from_trust(0), TrustStage::Acquaintance);
        assert_eq!(TrustStage::from_trust(19), TrustStage::Acquaintance);
    }

    #[test]
    fn test_trust_stage_friend() {
        assert_eq!(TrustStage::from_trust(20), TrustStage::Friend);
        assert_eq!(TrustStage::from_trust(49), TrustStage::Friend);
    }

    #[test]
    fn test_trust_stage_companion() {
        assert_eq!(TrustStage::from_trust(50), TrustStage::Companion);
        assert_eq!(TrustStage::from_trust(100), TrustStage::Companion);
    }

    #[test]
    fn test_trust_stage_display() {
        assert_eq!(TrustStage::Companion.to_string(), "Companion");
        assert_eq!(TrustStage::Stranger.display_name(), "Stranger");
    }

    // --- TrustEvent ---------------------------------------------------------

    #[test]
    fn test_trust_event_impacts() {
        assert_eq!(TrustEvent::Cooperate(10).impact(), 10);
        assert_eq!(TrustEvent::Defect(-15).impact(), -15);
        assert_eq!(TrustEvent::Betray(-30).impact(), -30);
        assert!(TrustEvent::Forgive(8).is_positive());
        assert!(TrustEvent::Ignore(-5).is_negative());
    }

    #[test]
    fn test_trust_event_default_impacts() {
        assert_eq!(TrustEvent::Cooperate(10).default_impact(), 10);
        assert_eq!(TrustEvent::Defect(-15).default_impact(), -15);
        assert_eq!(TrustEvent::Ignore(-5).default_impact(), -5);
        assert_eq!(TrustEvent::Forgive(8).default_impact(), 8);
        assert_eq!(TrustEvent::Betray(-30).default_impact(), -30);
    }

    // --- TrustRelation ------------------------------------------------------

    #[test]
    fn test_trust_relation_new() {
        let r = TrustRelation::new();
        assert_eq!(r.a_trusts_b, 0);
        assert_eq!(r.b_trusts_a, 0);
        assert_eq!(r.average(), 0.0);
    }

    #[test]
    fn test_trust_relation_apply_events() {
        let mut r = TrustRelation::new();
        r.apply_event_a_to_b(TrustEvent::Cooperate(10));
        assert_eq!(r.a_trusts_b, 10);
        r.apply_event_a_to_b(TrustEvent::Betray(-30));
        assert_eq!(r.a_trusts_b, -20);
        r.apply_event_b_to_a(TrustEvent::Cooperate(15));
        assert_eq!(r.b_trusts_a, 15);
    }

    #[test]
    fn test_trust_relation_clamped() {
        let mut r = TrustRelation::from_scores(95, -90);
        r.apply_event_a_to_b(TrustEvent::Cooperate(20)); // 95+20=115 -> clamped to 100
        assert_eq!(r.a_trusts_b, TRUST_MAX);
        r.apply_event_b_to_a(TrustEvent::Betray(-30)); // -90-30=-120 -> clamped to -100
        assert_eq!(r.b_trusts_a, TRUST_MIN);
    }

    #[test]
    fn test_trust_relation_decay() {
        let mut r = TrustRelation::from_scores(100, -100);
        r.decay(0.1);
        assert_eq!(r.a_trusts_b, 90);  // 100 - 10
        assert_eq!(r.b_trusts_a, -90); // -100 + 10
    }

    #[test]
    fn test_mutual_acquaintance() {
        let mut r = TrustRelation::from_scores(5, 5);
        assert!(r.is_mutual_acquaintance());
        r.a_trusts_b = -5;
        assert!(!r.is_mutual_acquaintance());
    }

    // --- TrustDecay ---------------------------------------------------------

    #[test]
    fn test_trust_decay_steps() {
        let mut d = TrustDecay::new(80, 0.5);
        d.step();
        assert_eq!(d.trust, 40); // 80 - 40
        d.step();
        assert_eq!(d.trust, 20); // 40 - 20
        d.step();
        assert_eq!(d.trust, 10);
    }

    #[test]
    fn test_trust_decay_to_zero() {
        let mut d = TrustDecay::new(1, 0.5);
        d.step();
        assert_eq!(d.trust, 0); // 1 - round(0.5) = 0
    }

    #[test]
    fn test_trust_decay_negative() {
        let mut d = TrustDecay::new(-60, 0.5);
        d.step();
        assert_eq!(d.trust, -30);
    }

    // --- ForgivenessConfig --------------------------------------------------

    #[test]
    fn test_forgiveness_applies() {
        let cfg = ForgivenessConfig::recommended(); // rate 0.6, threshold -50, cooldown 5
        let (new_trust, steps) = cfg.apply(-30, 10); // above cooldown
        assert!(new_trust > -30); // moved toward zero
        assert_eq!(steps, 0); // cooldown reset
    }

    #[test]
    fn test_forgiveness_cooldown_blocks() {
        let cfg = ForgivenessConfig::recommended();
        let (new_trust, _) = cfg.apply(-30, 2); // below cooldown of 5
        assert_eq!(new_trust, -30); // unchanged
    }

    #[test]
    fn test_forgiveness_threshold_blocks() {
        let cfg = ForgivenessConfig::recommended(); // threshold -50
        let (new_trust, _) = cfg.apply(-60, 10); // below threshold
        assert_eq!(new_trust, -60);
    }

    #[test]
    fn test_forgiveness_positive_ignored() {
        let cfg = ForgivenessConfig::recommended();
        let (new_trust, _) = cfg.apply(10, 10);
        assert_eq!(new_trust, 10); // positive trust not affected
    }

    // --- TrustNetwork -------------------------------------------------------

    #[test]
    fn test_network_basic() {
        let mut net = TrustNetwork::with_agents(&["alice", "bob", "carol"]);
        assert_eq!(net.agent_count(), 3);

        net.apply_event("alice", "bob", TrustEvent::Cooperate(20));
        let rel = net.trust_between("alice", "bob").unwrap();
        assert_eq!(rel.a_trusts_b, 20);

        // Reverse direction still 0
        assert_eq!(rel.b_trusts_a, 0);
    }

    #[test]
    fn test_network_most_least_trusted() {
        let mut net = TrustNetwork::with_agents(&["alice", "bob", "carol"]);
        net.apply_event("alice", "bob", TrustEvent::Cooperate(30));
        net.apply_event("carol", "bob", TrustEvent::Cooperate(20));
        assert_eq!(net.most_trusted().unwrap(), "bob");

        net.apply_event("alice", "carol", TrustEvent::Betray(-40));
        net.apply_event("bob", "carol", TrustEvent::Defect(-10));
        assert_eq!(net.least_trusted().unwrap(), "carol");
    }

    #[test]
    fn test_network_mutual_trust() {
        let mut net = TrustNetwork::with_agents(&["alice", "bob"]);
        net.apply_event("alice", "bob", TrustEvent::Cooperate(20));
        net.apply_event("bob", "alice", TrustEvent::Cooperate(10));
        let mt = net.mutual_trust("alice", "bob");
        assert_eq!(mt, 15.0); // (20 + 10) / 2
    }

    #[test]
    fn test_network_trust_circle() {
        let mut net = TrustNetwork::with_agents(&["alice", "bob", "carol", "dave"]);
        net.apply_event("alice", "bob", TrustEvent::Cooperate(30));
        net.apply_event("bob", "alice", TrustEvent::Cooperate(30));
        net.apply_event("alice", "carol", TrustEvent::Cooperate(10));
        net.apply_event("carol", "alice", TrustEvent::Cooperate(10));
        // dave stays neutral
        let circle = net.trust_circle("alice", 10.0);
        assert!(circle.contains(&"bob"));
        assert!(circle.contains(&"carol"));
        assert!(!circle.contains(&"dave"));
    }

    // --- ReputationScore ----------------------------------------------------

    #[test]
    fn test_reputation_average() {
        let rs = ReputationScore::new(&[10, 20, 30]);
        assert_eq!(rs.average(), 20.0);
    }

    #[test]
    fn test_reputation_weighted() {
        let rs = ReputationScore::new(&[10, 20, 30]);
        let weighted = rs.weighted_average(&[1.0, 2.0, 3.0]);
        // (10*1 + 20*2 + 30*3) / (1+2+3) = (10+40+90)/6 = 23.333...
        assert!((weighted - 23.333).abs() < 0.01);
    }

    #[test]
    fn test_reputation_min_max() {
        let rs = ReputationScore::new(&[-10, 25, 50, -5]);
        assert_eq!(rs.min(), -10);
        assert_eq!(rs.max(), 50);
    }

    #[test]
    fn test_reputation_consensus() {
        let rs = ReputationScore::new(&[10, -5, 0, 20, -10]);
        // 3 non-negative out of 5
        assert!((rs.consensus() - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_reputation_stage() {
        let rs = ReputationScore::new(&[50, 60, 55]);
        assert_eq!(rs.stage(), TrustStage::Companion);
    }

    #[test]
    fn test_reputation_empty() {
        let rs = ReputationScore::new(&[]);
        assert_eq!(rs.average(), 0.0);
        assert_eq!(rs.min(), 0);
        assert_eq!(rs.max(), 0);
    }
}
