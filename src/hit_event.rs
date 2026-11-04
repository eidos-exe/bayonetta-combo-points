pub fn qualifies(damage: f32, shielding: bool, invincible: bool) -> bool {
    damage.is_finite() && damage > 0.0 && !shielding && !invincible
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_bullet_and_melee_contacts_both_count() {
        for d in [0.2, 0.5, 1.4, 8.0, 15.0] {
            assert!(qualifies(d, false, false));
        }
    }
    #[test]
    fn shield_invincibility_and_invalid_damage_never_score() {
        assert!(!qualifies(10.0, true, false));
        assert!(!qualifies(10.0, false, true));
        for d in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(!qualifies(d, false, false));
        }
    }
}
