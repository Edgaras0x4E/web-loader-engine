use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{info, warn};

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationStrategy {
    Off,
    RoundRobin,
    Random,
}

impl RotationStrategy {
    fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "round_robin" | "round-robin" | "roundrobin" | "rr" => Self::RoundRobin,
            "random" | "rand" => Self::Random,
            "off" | "none" | "" => Self::Off,
            other => {
                warn!(
                    "Unknown USER_AGENT_ROTATION value '{}', defaulting to off",
                    other
                );
                Self::Off
            }
        }
    }
}

pub struct UserAgentService {
    default_ua: String,
    pool: Vec<String>,
    strategy: RotationStrategy,
    cursor: AtomicUsize,
}

impl UserAgentService {
    pub fn new(config: &Config) -> Self {
        let strategy = RotationStrategy::parse(&config.user_agent_rotation);
        let pool = config.user_agent_pool.clone();

        info!(
            "User agent service initialized (strategy={:?}, pool_size={})",
            strategy,
            pool.len()
        );
        if strategy != RotationStrategy::Off && pool.is_empty() {
            warn!("USER_AGENT_ROTATION is set but pool is empty, falling back to default user agent");
        }

        Self {
            default_ua: config.default_user_agent.clone(),
            pool,
            strategy,
            cursor: AtomicUsize::new(0),
        }
    }

    /// Resolve the user agent to use for a single request.
    ///
    /// Precedence:
    ///   - override == Some("default") → configured default
    ///   - override == Some("rotate")  → force rotation (round-robin if strategy is Off)
    ///   - override == Some(other)     → used verbatim
    ///   - None + strategy != Off      → rotation per strategy
    ///   - None + strategy == Off      → configured default
    pub fn resolve(&self, override_value: Option<&str>) -> String {
        match override_value.map(str::trim) {
            Some("default") => self.default_ua.clone(),
            Some("rotate") => self
                .rotate(if self.strategy == RotationStrategy::Off {
                    RotationStrategy::RoundRobin
                } else {
                    self.strategy
                })
                .unwrap_or_else(|| self.default_ua.clone()),
            Some(ua) if !ua.is_empty() => ua.to_string(),
            _ => {
                if self.strategy == RotationStrategy::Off {
                    self.default_ua.clone()
                } else {
                    self.rotate(self.strategy)
                        .unwrap_or_else(|| self.default_ua.clone())
                }
            }
        }
    }

    fn rotate(&self, strategy: RotationStrategy) -> Option<String> {
        if self.pool.is_empty() {
            return None;
        }
        let idx = match strategy {
            RotationStrategy::Off => return None,
            RotationStrategy::RoundRobin => {
                self.cursor.fetch_add(1, Ordering::Relaxed) % self.pool.len()
            }
            RotationStrategy::Random => {
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.subsec_nanos() as usize)
                    .unwrap_or(0);
                nanos % self.pool.len()
            }
        };
        self.pool.get(idx).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(default: &str, pool: Vec<&str>, rotation: &str) -> Config {
        Config {
            default_user_agent: default.to_string(),
            user_agent_pool: pool.into_iter().map(String::from).collect(),
            user_agent_rotation: rotation.to_string(),
            ..Config::default()
        }
    }

    #[test]
    fn returns_default_when_rotation_off_and_no_override() {
        let svc = UserAgentService::new(&cfg("DEFAULT", vec!["A", "B"], "off"));
        assert_eq!(svc.resolve(None), "DEFAULT");
    }

    #[test]
    fn round_robin_cycles_through_pool() {
        let svc = UserAgentService::new(&cfg("D", vec!["A", "B", "C"], "round_robin"));
        assert_eq!(svc.resolve(None), "A");
        assert_eq!(svc.resolve(None), "B");
        assert_eq!(svc.resolve(None), "C");
        assert_eq!(svc.resolve(None), "A");
    }

    #[test]
    fn explicit_ua_passes_through() {
        let svc = UserAgentService::new(&cfg("D", vec!["A"], "round_robin"));
        assert_eq!(svc.resolve(Some("CustomBot/1.0")), "CustomBot/1.0");
    }

    #[test]
    fn default_keyword_forces_default() {
        let svc = UserAgentService::new(&cfg("D", vec!["A"], "round_robin"));
        assert_eq!(svc.resolve(Some("default")), "D");
    }

    #[test]
    fn rotate_keyword_rotates_even_when_strategy_off() {
        let svc = UserAgentService::new(&cfg("D", vec!["A", "B"], "off"));
        let first = svc.resolve(Some("rotate"));
        let second = svc.resolve(Some("rotate"));
        assert!(["A", "B"].contains(&first.as_str()));
        assert!(["A", "B"].contains(&second.as_str()));
        assert_ne!(first, second);
    }

    #[test]
    fn empty_pool_falls_back_to_default() {
        let svc = UserAgentService::new(&cfg("D", vec![], "round_robin"));
        assert_eq!(svc.resolve(None), "D");
        assert_eq!(svc.resolve(Some("rotate")), "D");
    }

    #[test]
    fn rotation_strategy_parses_off_aliases() {
        assert_eq!(RotationStrategy::parse("off"), RotationStrategy::Off);
        assert_eq!(RotationStrategy::parse("OFF"), RotationStrategy::Off);
        assert_eq!(RotationStrategy::parse("none"), RotationStrategy::Off);
        assert_eq!(RotationStrategy::parse(""), RotationStrategy::Off);
        assert_eq!(RotationStrategy::parse("  "), RotationStrategy::Off);
    }

    #[test]
    fn rotation_strategy_parses_round_robin_aliases() {
        assert_eq!(
            RotationStrategy::parse("round_robin"),
            RotationStrategy::RoundRobin
        );
        assert_eq!(
            RotationStrategy::parse("round-robin"),
            RotationStrategy::RoundRobin
        );
        assert_eq!(
            RotationStrategy::parse("roundrobin"),
            RotationStrategy::RoundRobin
        );
        assert_eq!(RotationStrategy::parse("rr"), RotationStrategy::RoundRobin);
        assert_eq!(
            RotationStrategy::parse("Round_Robin"),
            RotationStrategy::RoundRobin
        );
    }

    #[test]
    fn rotation_strategy_parses_random_aliases() {
        assert_eq!(RotationStrategy::parse("random"), RotationStrategy::Random);
        assert_eq!(RotationStrategy::parse("rand"), RotationStrategy::Random);
        assert_eq!(RotationStrategy::parse("RANDOM"), RotationStrategy::Random);
    }

    #[test]
    fn rotation_strategy_unknown_defaults_to_off() {
        assert_eq!(RotationStrategy::parse("nonsense"), RotationStrategy::Off);
        assert_eq!(RotationStrategy::parse("shuffle"), RotationStrategy::Off);
    }

    #[test]
    fn random_strategy_returns_value_from_pool() {
        let svc = UserAgentService::new(&cfg("D", vec!["A", "B", "C"], "random"));
        for _ in 0..20 {
            let picked = svc.resolve(None);
            assert!(
                ["A", "B", "C"].contains(&picked.as_str()),
                "random strategy returned {:?}, expected one of pool",
                picked
            );
        }
    }

    #[test]
    fn empty_string_override_treated_as_none() {
        let svc = UserAgentService::new(&cfg("D", vec!["A", "B"], "round_robin"));
        assert_eq!(svc.resolve(Some("")), "A");
    }

    #[test]
    fn whitespace_only_override_treated_as_none() {
        let svc = UserAgentService::new(&cfg("D", vec!["A", "B"], "round_robin"));
        assert_eq!(svc.resolve(Some("   ")), "A");
    }

    #[test]
    fn whitespace_around_keyword_still_recognized() {
        let svc = UserAgentService::new(&cfg("D", vec!["A"], "round_robin"));
        assert_eq!(svc.resolve(Some("  default  ")), "D");
        let first = svc.resolve(Some("  rotate  "));
        assert_eq!(first, "A");
    }
}
