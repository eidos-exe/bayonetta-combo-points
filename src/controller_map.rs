// Smash 13.0.5 input-manager routing. Controller storage and the input
// manager are separate globals; player slots are not physical controller IDs.
// Reference: smash-gamepad-overlay/src/final_input.rs, read_player.
pub trait Reader {
    fn ptr(&self, address: usize) -> Option<usize>;
    fn word(&self, address: usize) -> i32;
    fn byte(&self, address: usize) -> u8;
}
pub fn local<R: Reader>(r: &R, base: usize, player: i32) -> Result<i32, String> {
    if !(0..8).contains(&player) {
        return Err(format!("player={} outside range", player));
    }
    let manager = r.ptr(base + 0x52c50f0).ok_or("input manager unavailable")?;
    if r.ptr(manager) != Some(base + 0x509e5a8) {
        return Err("input manager vtable mismatch".into());
    }
    let controller = r.word(manager + 0x298 + 4 * player as usize);
    if !(0..=9).contains(&controller) {
        return Err(format!(
            "player={} controller={} unassigned",
            player, controller
        ));
    }
    let mode = r
        .ptr(base + 0x53398b0)
        .ok_or("controller mode unavailable")?;
    let storage = if r.byte(mode + 8) != 0 {
        r.ptr(base + 0x5339860 + 8 * controller as usize)
            .ok_or("controller table entry unavailable")?
    } else {
        base + 0x5339830
    };
    let device = r
        .ptr(storage + 0x10)
        .ok_or("controller device unavailable")?;
    let valid = r.byte(device + 0xb8);
    let connected = r.byte(device + 0xbb);
    if valid == 0 || connected == 0 {
        return Err(format!(
            "player={} controller={} valid={} connected={}",
            player, controller, valid, connected
        ));
    }
    Ok(controller)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    struct Memory(BTreeMap<usize, usize>);
    impl Reader for Memory {
        fn ptr(&self, a: usize) -> Option<usize> {
            self.0
                .get(&a)
                .copied()
                .filter(|&n| n >= 0x1000 && n % 8 == 0)
        }
        fn word(&self, a: usize) -> i32 {
            self.0.get(&a).copied().unwrap_or(usize::MAX) as i32
        }
        fn byte(&self, a: usize) -> u8 {
            self.0.get(&a).copied().unwrap_or(0) as u8
        }
    }
    const B: usize = 0x10000000;
    const M: usize = 0x20000000;
    const MODE: usize = 0x30000000;
    const STORAGE: usize = 0x40000000;
    const DEVICE: usize = 0x50000000;
    fn setup(player: i32, controller: i32) -> Memory {
        Memory(
            [
                (B + 0x52c50f0, M),
                (M, B + 0x509e5a8),
                (M + 0x298 + 4 * player as usize, controller as usize),
                (B + 0x53398b0, MODE),
                (MODE + 8, 1),
                (B + 0x5339860 + 8 * controller as usize, STORAGE),
                (STORAGE + 0x10, DEVICE),
                (DEVICE + 0xb8, 1),
                (DEVICE + 0xbb, 1),
                (STORAGE + 0x298 + 4 * player as usize, 1134559232),
            ]
            .iter()
            .copied()
            .collect(),
        )
    }
    #[test]
    fn local_player_in_another_slot_routes_to_controller_zero() {
        assert_eq!(local(&setup(5, 0), B, 5), Ok(0));
    }
    #[test]
    fn remote_slot_is_rejected_without_claiming_another_controller() {
        let mut m = setup(1, 0);
        m.0.insert(M + 0x298, usize::MAX);
        assert!(local(&m, B, 0).is_err());
        assert_eq!(local(&m, B, 1), Ok(0));
    }
    #[test]
    fn fallback_controller_storage_is_inline() {
        let mut m = setup(2, 0);
        m.0.insert(MODE + 8, 0);
        m.0.insert(B + 0x5339830 + 0x10, DEVICE);
        assert_eq!(local(&m, B, 2), Ok(0));
    }
    #[test]
    fn disconnected_and_wrong_manager_are_rejected() {
        let mut m = setup(1, 0);
        m.0.insert(DEVICE + 0xbb, 0);
        assert!(local(&m, B, 1).is_err());
        m.0.insert(DEVICE + 0xbb, 1);
        m.0.insert(M, B + 0x1000);
        assert!(local(&m, B, 1).is_err());
    }
}
