#![feature(proc_macro_hygiene, repr_simd, simd_ffi, core_intrinsics)]
use smash::{
    app::{self, lua_bind::*, sv_battle_object, utility, BattleObjectModuleAccessor},
    lib::lua_const::*,
    lua2cpp::L2CFighterCommon,
    phx::{Hash40, Vector3f},
};
use smashline::{Agent, Main};
use skyline::hooks::{getRegionAddress, Region};
use std::{sync::Mutex, convert::TryInto};
mod hit_owner;
mod score;
mod classify;
mod hud;
mod native_vector;
mod events;
mod diagnostics;
mod hud_layout;
mod hit_event;
mod capture;
mod hud_animation;
mod hud_camera;
mod hud_owner;
mod trophy;
mod controller_map;
#[allow(dead_code)]
mod native_hud_tracks;
use events::Contact;
use score::{Category, Chain};

#[derive(serde::Deserialize)]
#[serde(default)]
struct Settings {
    hud_width_px: f32,
    trophy_thresholds: [u64; 5],
    multiplier_step_points: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            hud_width_px: 200.0,
            trophy_thresholds: trophy::DEFAULT_THRESHOLDS,
            multiplier_step_points: score::DEFAULT_MULTIPLIER_STEP,
        }
    }
}
lazy_static::lazy_static! {static ref SETTINGS:Mutex<Settings>=Mutex::new(Settings::default());}
pub static CHAINS: Mutex<[Chain; 8]> = Mutex::new([Chain::new(); 8]);
static mut COLLISION_OFFSET: usize = 0;
static CALLBACK_SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// Execute overlay placement in Smash's HUD-update phase, rather than the
// fighter-update phase before the camera has reached its current-frame pose.
// The hook location is the HUD update site used by HDR's native UI manager.
#[skyline::hook(offset = 0x138a710, inline)]
unsafe fn overlay_update(_: &skyline::hooks::InlineCtx) {
    static SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !SEEN.swap(true, std::sync::atomic::Ordering::Relaxed) {
        diagnostics::trace("HUD update callback active; persistent projected renderer");
    }
    static MATCH_FRAME: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    if !app::sv_information::is_ready_go() {
        MATCH_FRAME.store(0, std::sync::atomic::Ordering::Relaxed);
        hud::forget();
        return;
    }
    let match_frame = MATCH_FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let sample = matches!(match_frame, 0 | 30 | 120);
    let mut controller_snapshot = Vec::new();
    let mut candidates = Vec::new();
    let manager = singletons::FighterManager();
    if manager.is_null() {
        hud::retain_owner(None);
        return;
    }
    for slot in 0..8 {
        if let Some(b) = fighter(slot) {
            let info =
                FighterManager::get_fighter_information(manager, app::FighterEntryID(slot as i32));
            if info.is_null() {
                continue;
            }
            let cpu = FighterInformation::is_operation_cpu(info);
            let bayonetta = utility::get_kind(&mut *b) == *FIGHTER_KIND_BAYONETTA;
            let controller = if !cpu && bayonetta {
                local_controller(b)
            } else {
                None
            };
            if sample {
                controller_snapshot.push(format!(
                    "slot={} kind={} cpu={} probe={:?}",
                    slot,
                    utility::get_kind(&mut *b),
                    cpu,
                    controller_probe(b)
                ));
            }
            candidates.push(hud_owner::Candidate {
                slot,
                controller,
                cpu,
                bayonetta,
            });
        }
    }
    let selected = hud_owner::select(&candidates);
    if sample {
        diagnostics::match_event(&format!(
            "ownership frame={} selected={:?}; {}",
            match_frame,
            selected,
            controller_snapshot.join("; ")
        ));
    }
    static LAST_OWNER: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(usize::MAX);
    let owner_key = selected.unwrap_or(usize::MAX);
    if LAST_OWNER.swap(owner_key, std::sync::atomic::Ordering::Relaxed) != owner_key {
        diagnostics::trace(&format!(
            "local chart owner={:?} controllers={:?}",
            selected,
            candidates
                .iter()
                .map(|c| (c.slot, c.controller, c.cpu))
                .collect::<Vec<_>>()
        ));
    }
    hud::retain_owner(selected);
    if let Some(slot) = selected {
        if let Some(b) = fighter(slot) {
            let snapshot = CHAINS.lock().unwrap()[slot];
            hud::update(b, slot, snapshot, false);
        }
    }
}

// Read-only controller chain from HDR dynamic/src/ext.rs get_player_idx_from_boma.
// Remote/CPU input objects do not supply a valid local mapper index.
unsafe fn local_controller(b: *mut BattleObjectModuleAccessor) -> Option<i32> {
    controller_probe(b).ok()
}
unsafe fn controller_probe(b: *mut BattleObjectModuleAccessor) -> Result<i32, String> {
    unsafe fn pointer(base: *const u8, offset: usize) -> Option<*const u8> {
        let p = *(base.add(offset) as *const *const u8);
        if p as usize >= 0x1000 && (p as usize) % 8 == 0 {
            Some(p)
        } else {
            None
        }
    }
    let control = pointer(b as *const u8, 0x48).ok_or("control unavailable")?;
    let first = pointer(control, 0x118).ok_or("control input unavailable")?;
    let second = pointer(first, 0x58).ok_or("input context unavailable")?;
    let input = pointer(second, 0x8).ok_or("input mapper unavailable")?;
    let index = *(input.add(8) as *const i32);
    if !(0..8).contains(&index) {
        return Err(format!("mapper index {} outside local range", index));
    }
    struct NativeReader;
    impl controller_map::Reader for NativeReader {
        fn ptr(&self, a: usize) -> Option<usize> {
            unsafe {
                let p = (a as *const usize).read_volatile();
                if p >= 0x1000 && p % 8 == 0 {
                    Some(p)
                } else {
                    None
                }
            }
        }
        fn word(&self, a: usize) -> i32 {
            unsafe { (a as *const i32).read_volatile() }
        }
        fn byte(&self, a: usize) -> u8 {
            unsafe { (a as *const u8).read_volatile() }
        }
    }
    controller_map::local(
        &NativeReader,
        getRegionAddress(Region::Text) as usize,
        index,
    )
}

unsafe fn entry(b: *mut BattleObjectModuleAccessor) -> Option<usize> {
    if b.is_null() || utility::get_category(&mut *b) != *BATTLE_OBJECT_CATEGORY_FIGHTER {
        return None;
    }
    let i = WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID);
    if (0..8).contains(&i) {
        Some(i as usize)
    } else {
        None
    }
}
unsafe fn fighter(slot: usize) -> Option<*mut BattleObjectModuleAccessor> {
    let id = app::Fighter::get_id_from_entry_id(slot as i32);
    if !sv_battle_object::is_active(id) {
        return None;
    }
    let b = sv_battle_object::module_accessor(id);
    if entry(b) == Some(slot) {
        Some(b)
    } else {
        None
    }
}
unsafe fn resolve_owner(id: u32) -> Option<hit_owner::Owner> {
    hit_owner::fighter_owner(id, |current| {
        if current == u32::MAX
            || current == *BATTLE_OBJECT_ID_INVALID as u32
            || !sv_battle_object::is_active(current)
        {
            return None;
        }
        let b = sv_battle_object::module_accessor(current);
        if b.is_null() {
            return None;
        }
        match utility::get_category(&mut *b) {
            c if c == *BATTLE_OBJECT_CATEGORY_FIGHTER => Some(hit_owner::Source::Fighter),
            c if c == *BATTLE_OBJECT_CATEGORY_WEAPON => Some(hit_owner::Source::OwnedBy {
                owner: WorkModule::get_int(b, *WEAPON_INSTANCE_WORK_ID_INT_LINK_OWNER) as u32,
                item: false,
            }),
            c if c == *BATTLE_OBJECT_CATEGORY_ITEM => {
                let owner = if LinkModule::is_link(b, *ITEM_LINK_NO_HAVE) {
                    LinkModule::get_parent_id(b, *ITEM_LINK_NO_HAVE, true)
                } else {
                    TeamModule::team_owner_id(b)
                };
                if owner > u32::MAX as u64 {
                    return None;
                }
                Some(hit_owner::Source::OwnedBy {
                    owner: owner as u32,
                    item: true,
                })
            }
            _ => None,
        }
    })
}
unsafe fn source(
    id: u32,
) -> Option<(
    *mut BattleObjectModuleAccessor,
    *mut BattleObjectModuleAccessor,
)> {
    let resolved = resolve_owner(id)?;
    let owner = sv_battle_object::module_accessor(resolved.fighter);
    if owner.is_null()
        || entry(owner).is_none()
        || utility::get_kind(&mut *owner) != *FIGHTER_KIND_BAYONETTA
    {
        return None;
    }
    Some((owner, sv_battle_object::module_accessor(id)))
}
unsafe fn item_gunfire(b: *mut BattleObjectModuleAccessor) -> bool {
    if utility::get_category(&mut *b) == *BATTLE_OBJECT_CATEGORY_ITEM
        && [
            *ITEM_KIND_RAYGUNSHOT,
            *ITEM_KIND_SUPERSCOPESHOT,
            *ITEM_KIND_BANANAGUNSHOT,
            *ITEM_KIND_FIREFLOWERSHOT,
            *ITEM_KIND_DRILLSHOT,
            *ITEM_KIND_CLUBSHOT,
        ]
        .contains(&utility::get_kind(&mut *b))
    {
        return true;
    }
    for id in 0..10 {
        if AttackModule::is_attack(b, id, false) {
            let data = AttackModule::attack_data(b, id, false);
            if !data.is_null() && (*data).attr == Hash40::new("collision_attr_normal_bullet").hash {
                return true;
            }
        }
    }
    false
}
unsafe fn category(
    owner: *mut BattleObjectModuleAccessor,
    b: *mut BattleObjectModuleAccessor,
    damage: f32,
) -> Option<Category> {
    if owner != b {
        let source_category = utility::get_category(&mut *b);
        if source_category == *BATTLE_OBJECT_CATEGORY_WEAPON {
            match utility::get_kind(&mut *b) {
                k if k == *WEAPON_KIND_BAYONETTA_SPECIALN_BULLET => return Some(Category::Handgun),
                k if k == *WEAPON_KIND_BAYONETTA_WICKEDWEAVEARM => return Some(Category::Punch),
                k if k == *WEAPON_KIND_BAYONETTA_WICKEDWEAVELEG => return Some(Category::Kick),
                _ => {
                    if !resolve_owner((*b).battle_object_id)
                        .map(|o| o.via_item)
                        .unwrap_or(false)
                    {
                        return None;
                    }
                }
            }
        } else if source_category != *BATTLE_OBJECT_CATEGORY_ITEM {
            return None;
        }
        return Some(if item_gunfire(b) {
            Category::Handgun
        } else {
            Category::Punch
        });
    }
    let motion = MotionModule::motion_kind(b);
    let name = classify::MOTIONS
        .iter()
        .find(|m| Hash40::new(m).hash == motion)
        .copied()
        .unwrap_or("");
    let mut physical = false;
    let mut bullet = false;
    let mut physical_power = f32::INFINITY;
    let mut bullet_power = f32::INFINITY;
    for id in 0..10 {
        if AttackModule::is_attack(b, id, false) {
            let data = AttackModule::attack_data(b, id, false);
            if !data.is_null() {
                if (*data).attr == Hash40::new("collision_attr_normal_bullet").hash {
                    bullet = true;
                    bullet_power = bullet_power.min((*data).power);
                } else {
                    physical = true;
                    physical_power = physical_power.min((*data).power);
                }
            }
        }
    }
    let movement = classify::physical(name, MotionModule::frame(b)).or_else(|| {
        if ItemModule::is_have_item(b, 0) {
            Some(Category::Punch)
        } else {
            None
        }
    });
    if physical && bullet {
        classify::mixed_contact(damage, physical_power, bullet_power, movement)
    } else {
        classify::family(physical, bullet, movement)
    }
}
unsafe fn credit(c: Contact, weave: bool, witch_time: bool) {
    if let Some(owner) = fighter(c.source) {
        if (*owner).battle_object_id != c.source_object {
            return;
        }
    } else {
        return;
    }
    let step = SETTINGS.lock().unwrap().multiplier_step_points;
    let earned = {
        let mut chains = CHAINS.lock().unwrap();
        chains[c.source].configure_multiplier(step);
        chains[c.source].hit(c.category, weave, witch_time)
    };
    diagnostics::trace(&format!(
        "score slot={} category={:?} earned={}",
        c.source, c.category, earned
    ));
}
// Forward the original collision arguments unchanged, including the
// floating-point register used by the installed combo-effect hook.
#[skyline::hook(offset=COLLISION_OFFSET)]
unsafe fn collision(
    manager: u64,
    attacker: u32,
    defender: u32,
    damage_register: f32,
    move_type: u64,
    arg5: u64,
    move_type_again: u64,
) -> u64 {
    let target = if sv_battle_object::is_active(defender) {
        sv_battle_object::module_accessor(defender)
    } else {
        std::ptr::null_mut()
    };
    let candidate = if capture::absolute_in_progress() {
        None
    } else {
        entry(target).and_then(|victim|source(attacker).and_then(|(owner,hit_source)|entry(owner).and_then(|slot|{
        if slot==victim{return None;}
        let status=StatusModule::status_kind(target);
        let shielding=[*FIGHTER_STATUS_KIND_GUARD,*FIGHTER_STATUS_KIND_GUARD_ON,*FIGHTER_STATUS_KIND_GUARD_DAMAGE].contains(&status);
        let invincible=HitModule::get_whole(target,*HIT_STATUS_INVINCIBLE)!=0;
        let cat=category(owner,hit_source,damage_register);
        diagnostics::trace(&format!("native hit slot={} victim={} motion={:x} damage={:.3} category={:?} shield={}",slot,victim,MotionModule::motion_kind(owner),damage_register,cat,shielding));
        if !hit_event::qualifies(damage_register,shielding,invincible){return None;}
        cat.map(|c|{
            let weave=owner!=hit_source&&utility::get_category(&mut*hit_source)==*BATTLE_OBJECT_CATEGORY_WEAPON&&[*WEAPON_KIND_BAYONETTA_WICKEDWEAVEARM,*WEAPON_KIND_BAYONETTA_WICKEDWEAVELEG].contains(&utility::get_kind(&mut*hit_source));
            let witch=WorkModule::is_flag(owner,*FIGHTER_BAYONETTA_INSTANCE_WORK_ID_FLAG_WITCH_TIME);
            (Contact::new(slot,(*owner).battle_object_id,c),weave,witch)
        })
    })))
    };
    let result = original!()(
        manager,
        attacker,
        defender,
        damage_register,
        move_type,
        arg5,
        move_type_again,
    );
    if let Some((c, weave, witch)) = candidate {
        credit(c, weave, witch);
    }
    result
}
unsafe extern "C" fn frame(agent: &mut L2CFighterCommon) {
    let b = agent.module_accessor;
    let slot = match entry(b) {
        Some(s) => s,
        None => return,
    };
    capture::observe(b, slot);
    let status = StatusModule::status_kind(b);
    let reset = !app::sv_information::is_ready_go()
        || [
            *FIGHTER_STATUS_KIND_ENTRY,
            *FIGHTER_STATUS_KIND_DEAD,
            *FIGHTER_STATUS_KIND_REBIRTH,
        ]
        .contains(&status);
    if utility::get_kind(&mut *b) != *FIGHTER_KIND_BAYONETTA {
        hud::clear(b, slot);
        CHAINS.lock().unwrap()[slot].clear();
        return;
    }
    if !CALLBACK_SEEN.swap(true, std::sync::atomic::Ordering::Relaxed) {
        diagnostics::trace(&format!(
            "Bayonetta frame callback active slot={} ready={}",
            slot, !reset
        ));
    }
    let step = SETTINGS.lock().unwrap().multiplier_step_points;
    let mut chains = CHAINS.lock().unwrap();
    let c = &mut chains[slot];
    c.configure_multiplier(step);
    if reset {
        c.clear();
    } else {
        c.tick_defense(
            StopModule::is_stop(b),
            WorkModule::is_flag(b, *FIGHTER_BAYONETTA_INSTANCE_WORK_ID_FLAG_WITCH_TIME),
            [
                *FIGHTER_BAYONETTA_STATUS_KIND_BATWITHIN,
                *FIGHTER_BAYONETTA_STATUS_KIND_SPECIAL_LW_BATWITHIN,
            ]
            .contains(&status),
        );
    }
    drop(chains);
    if !reset {
        hud::prepare(b, slot);
    }
}
#[skyline::main(name = "bayonetta_combo_points")]
pub fn main() {
    if let Ok(folders) = std::fs::read_dir("sd:/ultimate/mods") {
        for f in folders.flatten() {
            if let Ok(text) = std::fs::read_to_string(f.path().join("bayonetta_combo.toml")) {
                if let Ok(mut settings) = toml::from_str::<Settings>(&text) {
                    if !trophy::valid_thresholds(settings.trophy_thresholds) {
                        settings.trophy_thresholds = trophy::DEFAULT_THRESHOLDS;
                    }
                    *SETTINGS.lock().unwrap() = settings;
                    diagnostics::init(&f.path());
                    break;
                }
            }
        }
    }
    unsafe {
        let start = getRegionAddress(Region::Text) as *const u8;
        let size = getRegionAddress(Region::Rodata) as usize - start as usize;
        let text = std::slice::from_raw_parts(start, size);
        let mut display = skyline::nn::oe::DisplayVersion { name: [0; 16] };
        skyline::nn::oe::GetDisplayVersion(&mut display);
        let count = display.name.iter().position(|b| *b == 0).unwrap_or(16);
        let version = String::from_utf8_lossy(&display.name[..count]);
        let offset = 0x67a7b0;
        if matches!(version.as_ref(), "13.0.3" | "13.0.4" | "13.0.5") && text.len() > offset + 8 {
            let a = u32::from_le_bytes(text[offset..offset + 4].try_into().unwrap());
            let next = u32::from_le_bytes(text[offset + 4..offset + 8].try_into().unwrap());
            if a & 0xffc003ff == 0xd10003ff
                || a & 0xfc000000 == 0x14000000
                || (a & 0xff000000 == 0x58000000 && next & 0xfffffc1f == 0xd61f0000)
            {
                COLLISION_OFFSET = offset;
                skyline::install_hook!(collision);
            }
        }
        if matches!(version.as_ref(), "13.0.3" | "13.0.4" | "13.0.5") && text.len() > 0x138a714 {
            skyline::install_hook!(overlay_update);
            skyline::install_hooks!(capture::absolute, capture::absolute_joint);
        }
        // Other damage observers may scan the unmodified received-damage
        // prologue. Patching it early can prevent their hooks from initializing.
        // Collision contacts provide our feedback without patching the shared
        // received-damage entry a second time.
        diagnostics::trace(&format!(
            "loaded game={} collision_hook={}; received-damage entry untouched",
            version,
            COLLISION_OFFSET != 0
        ));
    }
    Agent::new("fighter").on_line(Main, frame).install();
}
