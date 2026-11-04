use super::*;
static OWNERS: Mutex<[Option<u32>; 8]> = Mutex::new([None; 8]);
static mut ABSOLUTE_DEPTH: u32 = 0;
pub unsafe fn absolute_in_progress() -> bool {
    ABSOLUTE_DEPTH != 0
}
pub unsafe fn observe(b: *mut BattleObjectModuleAccessor, slot: usize) {
    let owner = if app::sv_information::is_ready_go() && CaptureModule::is_capture(b) {
        Some(CaptureModule::catch_object_id(b) as u32)
    } else {
        None
    };
    let previous = {
        let mut owners = OWNERS.lock().unwrap();
        let old = owners[slot];
        owners[slot] = owner;
        old
    };
    if owner == previous {
        return;
    }
    if let Some(id) = owner {
        if let Some((o, _)) = source(id) {
            if let Some(k) = entry(o) {
                let status = StatusModule::status_kind(o);
                if [
                    *FIGHTER_STATUS_KIND_CATCH,
                    *FIGHTER_STATUS_KIND_CATCH_DASH,
                    *FIGHTER_STATUS_KIND_CATCH_TURN,
                    *FIGHTER_STATUS_KIND_CATCH_PULL,
                    *FIGHTER_STATUS_KIND_CATCH_DASH_PULL,
                    *FIGHTER_STATUS_KIND_CATCH_WAIT,
                    *FIGHTER_STATUS_KIND_CATCH_ATTACK,
                    *FIGHTER_STATUS_KIND_THROW,
                ]
                .contains(&status)
                {
                    credit(
                        Contact::new(k, (*o).battle_object_id, Category::Punch),
                        false,
                        false,
                    );
                    diagnostics::trace("successful capture credited once");
                }
            }
        }
    }
}
unsafe fn before(
    b: *mut BattleObjectModuleAccessor,
    kind: i32,
    target: u32,
) -> Option<(Contact, *mut BattleObjectModuleAccessor, f32)> {
    if ABSOLUTE_DEPTH != 0 || kind != *FIGHTER_ATTACK_ABSOLUTE_KIND_THROW {
        return None;
    }
    let (owner, _) = source((*b).battle_object_id)?;
    let k = entry(owner)?;
    if !sv_battle_object::is_active(target) {
        return None;
    }
    let victim = sv_battle_object::module_accessor(target);
    entry(victim)?;
    let m = MotionModule::motion_kind(owner);
    let cat = if m == Hash40::new("throw_f").hash {
        Category::Punch
    } else {
        Category::Kick
    };
    Some((
        Contact::new(k, (*owner).battle_object_id, cat),
        victim,
        DamageModule::damage(victim, 0),
    ))
}
unsafe fn after(pending: Option<(Contact, *mut BattleObjectModuleAccessor, f32)>) {
    if let Some((c, victim, old)) = pending {
        let now = DamageModule::damage(victim, 0);
        if now.is_finite() && now > old {
            credit(c, false, false);
            diagnostics::trace("absolute throw damage credited");
        }
    }
}
#[skyline::hook(replace=AttackModule::hit_absolute)]
pub unsafe fn absolute(
    b: *mut BattleObjectModuleAccessor,
    kind: i32,
    target: u32,
    pos: *const Vector3f,
    group: i32,
    no: i32,
) -> u64 {
    let pending = before(b, kind, target);
    ABSOLUTE_DEPTH += 1;
    let result = original!()(b, kind, target, pos, group, no);
    ABSOLUTE_DEPTH -= 1;
    after(pending);
    result
}
#[skyline::hook(replace=AttackModule::hit_absolute_joint)]
pub unsafe fn absolute_joint(
    b: *mut BattleObjectModuleAccessor,
    kind: i32,
    target: u32,
    joint: Hash40,
    group: i32,
    no: i32,
) -> u64 {
    let pending = before(b, kind, target);
    ABSOLUTE_DEPTH += 1;
    let result = original!()(b, kind, target, joint, group, no);
    ABSOLUTE_DEPTH -= 1;
    after(pending);
    result
}
