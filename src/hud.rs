use super::*;
// Persistent overlay components. Do not rebuild the header every few frames if it has not changed.
struct Handle {
    id: u32,
    name: String,
}
struct Pending {
    name: String,
    resource: String,
    position: [f32; 3],
    scale: f32,
    header: bool,
}
struct View {
    owner: Option<usize>,
    object: usize,
    handles: Vec<Handle>,
    pending: Vec<Pending>,
    failed_requests: Vec<String>,
    pool: hud_animation::Pool,
    mult: u32,
    header_age: u32,
    payout: trophy::Payout,
    chain_visible: bool,
}
static VIEW: Mutex<View> = Mutex::new(View {
    owner: None,
    object: 0,
    handles: Vec::new(),
    pending: Vec::new(),
    failed_requests: Vec::new(),
    pool: hud_animation::Pool::new(),
    mult: 0,
    header_age: 50,
    payout: trophy::Payout::new(),
    chain_visible: false,
});
pub fn forget() {
    let mut v = VIEW.lock().unwrap();
    v.owner = None;
    v.object = 0;
    v.handles.clear();
    v.pending.clear();
    v.failed_requests.clear();
    v.pool.reset();
    v.mult = 0;
    v.header_age = 50;
    v.payout = trophy::Payout::new();
    v.chain_visible = false;
}
unsafe fn remove(b: *mut BattleObjectModuleAccessor, v: &mut View) {
    v.pending.clear();
    v.failed_requests.clear();
    for h in v.handles.drain(..) {
        if EffectModule::is_exist_effect(b, h.id) {
            EffectModule::kill(b, h.id, true, true);
        }
    }
}
pub unsafe fn clear(b: *mut BattleObjectModuleAccessor, slot: usize) {
    let mut v = VIEW.lock().unwrap();
    if v.owner == Some(slot) {
        remove(b, &mut v);
        v.owner = None;
        v.object = 0;
        v.pool.reset();
        v.mult = 0;
        v.header_age = 50;
        v.payout = trophy::Payout::new();
        v.chain_visible = false;
    }
}
pub unsafe fn retain_owner(selected: Option<usize>) {
    let mut v = VIEW.lock().unwrap();
    if v.owner.is_some() && v.owner != selected {
        if let Some(old) = v.owner {
            if let Some(b) = fighter(old) {
                if b as usize == v.object {
                    remove(b, &mut v);
                }
            }
        }
        v.handles.clear();
        v.pending.clear();
        v.failed_requests.clear();
        v.owner = None;
        v.object = 0;
        v.pool.reset();
        v.mult = 0;
        v.header_age = 50;
        v.payout = trophy::Payout::new();
        v.chain_visible = false;
    }
}
fn digits(names: &mut Vec<String>, prefix: &str, value: u32, width: usize, zeros: bool) {
    let value = value.min(10u32.pow(width as u32) - 1);
    let text = if zeros {
        format!("{:0width$}", value, width = width)
    } else {
        format!("{:width$}", value, width = width)
    };
    for (i, ch) in text.chars().enumerate() {
        if ch.is_ascii_digit() {
            names.push(format!("sys_bayo_{}_{}_d{}", prefix, i, ch));
        }
    }
}
fn row_index(name: &str) -> Option<usize> {
    name.strip_prefix("sys_bayo_row")?
        .split('_')
        .next()?
        .parse()
        .ok()
}
pub unsafe fn update(b: *mut BattleObjectModuleAccessor, slot: usize, chain: Chain, reset: bool) {
    if reset {
        clear(b, slot);
        return;
    }
    let mut v = VIEW.lock().unwrap();
    if v.owner.is_some() && v.owner != Some(slot) {
        return;
    }
    if v.object != b as usize {
        v.handles.clear();
        v.pending.clear();
        v.failed_requests.clear();
        v.pool.reset();
        v.mult = 0;
        v.header_age = 50;
        v.payout.reset(&chain);
        v.chain_visible = false;
    }
    v.owner = Some(slot);
    v.object = b as usize;
    v.payout
        .observe(&chain, SETTINGS.lock().unwrap().trophy_thresholds);
    if chain.active() && v.mult != chain.multiplier_tenths() {
        v.header_age = hud_animation::multiplier_start_age(v.mult);
        v.mult = chain.multiplier_tenths();
    }
    let opening = chain.active() && !v.chain_visible;
    v.chain_visible = chain.active();
    v.pool.sync(&chain.rows);
    if opening {
        v.pool.settle_opening();
        diagnostics::trace(
            "chart opening: header and initial attack rows settled; subsequent rows animate",
        );
    }
    if !chain.active() && !v.pool.active() {
        v.mult = 0;
    }
    // Prewarm the stationary header while hidden. Only attack rows get entry motion.
    let header_points = if chain.active() {
        chain.points
    } else {
        v.payout.points
    };
    let header_mult = if chain.active() {
        chain.multiplier_tenths()
    } else {
        v.payout.multiplier
    };
    let mut names = vec![
        "sys_bayo_panel".to_owned(),
        "sys_bayo_payoutpanel".to_owned(),
    ];
    digits(&mut names, "total", header_points, 4, true);
    digits(&mut names, "mult", header_mult, 2, true);
    let reward_text = v.payout.score.to_string();
    if v.payout.age.is_some() {
        names.push(format!("sys_bayo_medal{}", v.payout.medal));
        for (i, ch) in reward_text.chars().enumerate() {
            names.push(format!("sys_bayo_reward_{}_d{}", i, ch));
        }
    }
    for (i, visual) in v.pool.rows.iter().enumerate() {
        if let Some(visual) = visual {
            let row = visual.row;
            names.push(format!("sys_bayo_row{}_frame", i));
            names.push(format!("sys_bayo_row{}_icon{}", i, row.category.icon()));
            digits(&mut names, &format!("row{}", i), row.points, 4, false);
            if row.witch_time {
                names.push(format!("sys_bayo_row{}_witch", i));
            }
        }
    }
    // Remove only obsolete glyphs/rows; unchanged names retain their particles.
    v.pending.retain(|request| names.contains(&request.name));
    v.handles.retain(|h| {
        if names.contains(&h.name) {
            true
        } else {
            if EffectModule::is_exist_effect(b, h.id) {
                EffectModule::kill(b, h.id, true, true);
            }
            false
        }
    });
    let width = SETTINGS.lock().unwrap().hud_width_px;
    let bounds = hud_camera::bounds();
    for name in names {
        let mut box_px = match hud_layout::component(&name) {
            Some(p) => p,
            None => continue,
        };
        let mut alpha = if row_index(&name).is_some() || chain.active() {
            1.0
        } else {
            v.payout.header_alpha()
        };
        let mut color = [1.0; 3];
        let mut scale = 1.0;
        let reward = name == "sys_bayo_payoutpanel"
            || name.starts_with("sys_bayo_reward_")
            || name.starts_with("sys_bayo_medal");
        let mut reward_scale = [1.0; 2];
        if reward {
            alpha = v.payout.reward_alpha();
            if name != "sys_bayo_payoutpanel" {
                reward_scale = v.payout.glyph_scale();
                if name.starts_with("sys_bayo_reward_") {
                    let i = name
                        .split('_')
                        .nth(3)
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(0);
                    let width = (126.0 / reward_text.len() as f32).min(18.0);
                    box_px[0] = 198.0 - (reward_text.len() - i) as f32 * width;
                    box_px[2] = width;
                    box_px[0] =
                        198.0 + (box_px[0] + width / 2.0 - 198.0) * reward_scale[0] - width / 2.0;
                    reward_scale[0] *= width / 18.0;
                    reward_scale[1] *= width / 18.0;
                    color = [335.0 / 255.0, 250.0 / 255.0, 220.0 / 255.0];
                }
            }
        }
        if let Some(i) = row_index(&name) {
            if let Some(visual) = v.pool.rows[i] {
                let (dx, _) = hud_animation::entry(visual.age);
                box_px[0] += dx;
                box_px[1] += 46.0 * (visual.y - i as f32);
                let appearance = hud_animation::appearance(visual);
                scale = appearance.0;
                alpha = appearance.1;
                color = appearance.2;
                if name.ends_with("_icon11") && visual.refreshed {
                    scale *= hud_animation::scalar(
                        native_hud_tracks::T901_SCALE,
                        visual.flash as f32,
                        false,
                    );
                }
                if name.ends_with("_witch") {
                    color =
                        hud_animation::rgb(native_hud_tracks::TWITCH_RGB, (visual.age % 30) as f32);
                }
            }
        }
        if name.contains("_total_") {
            color = [335.0 / 255.0, 250.0 / 255.0, 220.0 / 255.0];
        }
        if name.contains("_mult_") {
            color = hud_animation::rgb(native_hud_tracks::TMULT_RGB, v.header_age as f32);
            scale =
                hud_animation::scalar(native_hud_tracks::TMULT_SCALE, v.header_age as f32, false)
                    / 0.3;
            box_px[0] = 193.0 + (box_px[0] - 193.0) * scale;
            box_px[1] = 44.0 + (box_px[1] - 44.0) * scale;
            box_px[2] *= scale;
            box_px[3] *= scale;
            scale = 1.0;
        }
        let p = match hud_layout::placement_width(bounds, box_px, width) {
            Some(p) => p,
            None => continue,
        };
        let depth = if name.ends_with("_frame") || name.ends_with("panel") {
            0.0
        } else {
            0.02
        };
        let pos = native_vector::JointVector::new([p.x, p.y, depth]);
        let existing = v
            .handles
            .iter()
            .find(|h| h.name == name)
            .map(|h| h.id)
            .filter(|h| EffectModule::is_exist_effect(b, *h));
        let id = if let Some(id) = existing {
            id
        } else {
            if !v.pending.iter().any(|request| request.name == name) {
                v.pending.push(Pending {
                    name: name.clone(),
                    resource: hud_layout::effect_name(&name),
                    position: [p.x, p.y, depth],
                    scale: p.scale_x * scale * reward_scale[0].max(0.001),
                    header: row_index(&name).is_none(),
                });
            }
            continue;
        };
        EffectModule::set_pos(b, id, &pos as *const _ as *const Vector3f);
        EffectModule::set_scale(
            b,
            id,
            &Vector3f {
                x: p.scale_x * scale * reward_scale[0],
                y: p.scale_y * scale * reward_scale[1],
                z: 1.0,
            },
        );
        EffectModule::set_rgb(b, id, color[0], color[1], color[2]);
        EffectModule::set_alpha(b, id, alpha);
        // Hidden prewarmed panels must not cover the score or the next chain.
        EffectModule::set_visible(b, id, alpha > 0.001 && (reward || !v.payout.hides_chart()));
    }
    v.pool.advance();
    v.payout.advance(StopModule::is_stop(b));
    v.header_age = v.header_age.saturating_add(1);
}

// Fighter-owned effect resources are created in the fighter update. The HUD
// hook only positions and styles existing instances using the current camera.
pub unsafe fn prepare(b: *mut BattleObjectModuleAccessor, slot: usize) {
    let mut v = VIEW.lock().unwrap();
    if v.owner != Some(slot) || v.object != b as usize {
        return;
    }
    let pending = std::mem::take(&mut v.pending);
    for request in pending {
        if v.handles
            .iter()
            .any(|h| h.name == request.name && EffectModule::is_exist_effect(b, h.id))
        {
            continue;
        }
        v.handles.retain(|h| h.name != request.name);
        let position = native_vector::JointVector::new(request.position);
        let zero = native_vector::JointVector::new([0.0; 3]);
        let id = EffectModule::req(
            b,
            Hash40::new(&request.resource),
            &position as *const _ as *const Vector3f,
            &zero as *const _ as *const Vector3f,
            request.scale,
            0,
            -1,
            false,
            0,
        ) as u32;
        if !EffectModule::is_exist_effect(b, id) {
            if !v.failed_requests.contains(&request.resource) {
                diagnostics::trace(&format!(
                    "fighter-owned chart request failed resource={} costume={}",
                    request.resource,
                    WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_COLOR)
                ));
                v.failed_requests.push(request.resource);
            }
            continue;
        }
        if request.header {
            EffectModule::set_disable_render_offset_last(b);
            EffectModule::enable_sync_init_pos_last(b);
        }
        EffectModule::set_billboard(b, id, true);
        // Keep a prewarmed component invisible until the HUD applies its style.
        EffectModule::set_alpha(b, id, 0.0);
        EffectModule::set_visible(b, id, false);
        if request.name == "sys_bayo_panel" {
            diagnostics::trace(&format!(
                "fighter-owned chart panel created handle={} costume={}",
                id,
                WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_COLOR)
            ));
        }
        v.handles.push(Handle {
            id,
            name: request.name,
        });
    }
}
