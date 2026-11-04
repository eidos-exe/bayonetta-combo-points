// Payout phases traced from Bayonetta PS3 e28520/e287d8/e28ad0/e2a208.
// The reward is adapted: the full combo score stays visible beside its medal.
use crate::score::Chain;
pub const DEFAULT_THRESHOLDS: [u64; 5] = [25, 60, 110, 170, 250];
pub fn rank(score: u64, thresholds: [u64; 5]) -> usize {
    thresholds.iter().filter(|&&n| score >= n).count()
}
pub fn valid_thresholds(t: [u64; 5]) -> bool {
    t[0] > 0 && t.windows(2).all(|w| w[0] < w[1])
}
#[derive(Clone, Copy)]
pub struct Payout {
    pub age: Option<u32>,
    pub score: u64,
    pub medal: usize,
    pub points: u32,
    pub multiplier: u32,
    seen: u32,
}
impl Payout {
    pub const fn new() -> Self {
        Self {
            age: None,
            score: 0,
            medal: 0,
            points: 0,
            multiplier: 10,
            seen: 0,
        }
    }
    pub fn reset(&mut self, chain: &Chain) {
        *self = Self::new();
        self.seen = chain.settlement_id;
    }
    pub fn observe(&mut self, chain: &Chain, thresholds: [u64; 5]) {
        if chain.active() {
            self.points = chain.points;
            self.multiplier = chain.multiplier_tenths();
            self.age = None;
            self.seen = chain.settlement_id;
            return;
        }
        if self.seen != chain.settlement_id {
            self.seen = chain.settlement_id;
            self.score = chain.last_settlement;
            self.medal = rank(self.score, thresholds);
            self.age = if self.score > 0 { Some(0) } else { None };
        }
    }
    pub fn hides_chart(&self) -> bool {
        matches!(self.age,Some(age)if age>=5)
    }
    pub fn header_alpha(&self) -> f32 {
        self.age
            .filter(|&a| a < 5)
            .map(|a| 1.0 - 0.2 * a as f32)
            .unwrap_or(0.0)
    }
    pub fn reward_alpha(&self) -> f32 {
        match self.age {
            Some(a) if a >= 5 => ((a - 5) as f32 * 0.2).min(1.0),
            _ => 0.0,
        }
    }
    pub fn glyph_scale(&self) -> [f32; 2] {
        [self.reward_alpha() * 1.3, 1.3]
    }
    pub fn duration(&self) -> u32 {
        let step = (self.score / 185 + u64::from(self.score % 185 != 0)).max(1);
        40 + (self.score / step + u64::from(self.score % step != 0)) as u32
    }
    pub fn advance(&mut self, paused: bool) {
        if paused {
            return;
        }
        if let Some(age) = self.age {
            self.age = if age + 1 >= self.duration() {
                None
            } else {
                Some(age + 1)
            };
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::Category;
    fn end(c: &mut Chain) {
        while c.active() {
            c.tick(false);
        }
    }
    #[test]
    fn every_medal_boundary() {
        for (s, r) in [
            (1, 0),
            (24, 0),
            (25, 1),
            (59, 1),
            (60, 2),
            (109, 2),
            (110, 3),
            (169, 3),
            (170, 4),
            (249, 4),
            (250, 5),
        ] {
            assert_eq!(rank(s, DEFAULT_THRESHOLDS), r);
        }
    }
    #[test]
    fn full_multiplier_score_is_used() {
        let mut c = Chain::new();
        c.hit(Category::Punch, false, false);
        c.points = 150;
        let mut p = Payout::new();
        p.observe(&c, DEFAULT_THRESHOLDS);
        end(&mut c);
        p.observe(&c, DEFAULT_THRESHOLDS);
        assert_eq!(p.score, 240);
        assert_eq!(p.medal, 4);
        assert_eq!(p.points, 150);
    }
    #[test]
    fn equal_consecutive_rewards_trigger_once_each() {
        let mut c = Chain::new();
        let mut p = Payout::new();
        for id in 1..=2 {
            c.hit(Category::Punch, false, false);
            p.observe(&c, DEFAULT_THRESHOLDS);
            end(&mut c);
            p.observe(&c, DEFAULT_THRESHOLDS);
            assert_eq!(c.settlement_id, id);
            assert_eq!(p.age, Some(0));
            p.advance(false);
            p.observe(&c, DEFAULT_THRESHOLDS);
            assert_eq!(p.age, Some(1));
        }
    }
    #[test]
    fn source_transition_and_timeout() {
        let mut p = Payout::new();
        p.score = 800;
        p.age = Some(0);
        assert_eq!(p.header_alpha(), 1.0);
        assert_eq!(p.reward_alpha(), 0.0);
        p.age = Some(5);
        assert_eq!(p.header_alpha(), 0.0);
        assert_eq!(p.glyph_scale()[0], 0.0);
        p.age = Some(10);
        assert_eq!(p.reward_alpha(), 1.0);
        assert_eq!(p.glyph_scale(), [1.3, 1.3]);
        p.age = Some(p.duration() - 1);
        p.advance(false);
        assert_eq!(p.age, None);
    }
    #[test]
    fn new_hits_replace_old_payout_without_queueing() {
        let mut c = Chain::new();
        let mut p = Payout::new();
        c.hit(Category::Punch, false, false);
        end(&mut c);
        p.observe(&c, DEFAULT_THRESHOLDS);
        c.hit(Category::Kick, false, false);
        p.observe(&c, DEFAULT_THRESHOLDS);
        assert_eq!(p.age, None);
    }
    #[test]
    fn reset_does_not_replay_an_old_settlement() {
        let mut c = Chain::new();
        c.hit(Category::Punch, false, false);
        end(&mut c);
        let mut p = Payout::new();
        p.reset(&c);
        p.observe(&c, DEFAULT_THRESHOLDS);
        assert_eq!(p.age, None);
    }
    #[test]
    fn paused_payout_and_invalid_thresholds() {
        let mut p = Payout::new();
        p.age = Some(8);
        p.advance(true);
        assert_eq!(p.age, Some(8));
        assert!(!valid_thresholds([50, 50, 200, 400, 800]));
        assert!(valid_thresholds(DEFAULT_THRESHOLDS));
    }
}

#[cfg(test)]
mod tuned_reward_tests {
    #[test]
    fn agreed_pure_platinum_example() {
        let mut c = crate::score::Chain::new();
        c.hit(crate::score::Category::Punch, false, false);
        c.points = 157;
        while c.active() {
            c.tick(false);
        }
        assert_eq!(c.last_settlement, 251);
        assert_eq!(super::rank(c.last_settlement, super::DEFAULT_THRESHOLDS), 5);
    }
}

#[cfg(test)]
mod visibility_tests {
    use super::*;
    use crate::score::Category;
    #[test]
    fn payout_takes_over_after_header_fade_and_releases_on_next_chain() {
        let mut c = Chain::new();
        let mut p = Payout::new();
        c.hit(Category::Punch, false, false);
        p.observe(&c, DEFAULT_THRESHOLDS);
        assert!(!p.hides_chart());
        while c.active() {
            c.tick(false);
        }
        p.observe(&c, DEFAULT_THRESHOLDS);
        for age in 0..5 {
            assert_eq!(p.age, Some(age));
            assert!(!p.hides_chart());
            p.advance(false);
        }
        assert!(p.hides_chart());
        assert_eq!(p.header_alpha(), 0.0);
        for _ in 0..5 {
            p.advance(false);
        }
        assert_eq!(p.reward_alpha(), 1.0);
        assert!(p.hides_chart());
        c.hit(Category::Kick, false, false);
        p.observe(&c, DEFAULT_THRESHOLDS);
        assert!(!p.hides_chart());
        assert_eq!(p.reward_alpha(), 0.0);
    }
}
