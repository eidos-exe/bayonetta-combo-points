#[derive(Clone,Copy,Debug)]pub struct Placement {pub x:f32,pub y:f32,pub scale_x:f32,pub scale_y:f32}
pub fn effect_name(name:&str)->String {
    let logical=if name.starts_with("sys_bayo_reward_") {
        format!("sys_bayo_reward_{}",name.rsplit('_').next().unwrap_or("d0"))
    } else if name.starts_with("sys_bayo_row") {
        format!("sys_bayo_row0_{}",name.splitn(4,'_').last().unwrap_or(""))
    } else {name.to_owned()};
    format!("bayonetta_{}",logical.strip_prefix("sys_").unwrap_or(&logical))
}

pub fn placement(bounds:[f32;4],box_px:[f32;4])->Option<Placement>{
    placement_width(bounds,box_px,200.0)
}
pub fn placement_width(bounds:[f32;4],box_px:[f32;4],width_px:f32)->Option<Placement>{
    let [left,right,top,bottom]=bounds;let width=right-left;let height=top-bottom;
    if !bounds.iter().all(|v|v.is_finite())||width<1.0||height<1.0{return None;}
    let [x,y,w,h]=box_px;
    if !width_px.is_finite(){return None;}
    let width_px=width_px.max(160.0).min(240.0);let hud_scale=width_px/291.0;
    Some(Placement{x:left+width*((1280.0-32.0-width_px+(x+w/2.0)*hud_scale)/1280.0),y:top-height*((84.0+(y+h/2.0)*hud_scale)/720.0),scale_x:width/128.0*hud_scale,scale_y:height/72.0*hud_scale})
}
pub fn component(name:&str)->Option<[f32;4]>{
    let s=name.strip_prefix("sys_bayo_")?;
    if s=="payoutpanel"{return Some([0.0,0.0,291.0,210.0]);}if s.starts_with("medal"){return Some([212.0,28.0,32.0,32.0]);}if s.starts_with("reward_"){return Some([0.0,28.0,18.0,32.0]);}
    if s=="panel"{return Some([0.0,0.0,291.0,210.0]);}
    let p:Vec<_>=s.split('_').collect();
    if p[0]=="total"{let i=p.get(1)?.parse::<usize>().ok()?;if i<4{return Some([70.0+18.0*i as f32,29.0,18.0,32.0]);}}
    if p[0]=="mult"{let i=p.get(1)?.parse::<usize>().ok()?;if i<2{return Some([[174.0,195.0][i],32.0,14.0,25.0]);}}
    if p[0].starts_with("row"){let row=p[0].strip_prefix("row")?.parse::<usize>().ok()?;if row>=16{return None;}
        if *p.get(1)?=="witch"{return Some([245.0,80.0+46.0*row as f32,25.0,25.0]);}
        if *p.get(1)?=="frame"||*p.get(1)?=="glow"{return Some([22.0,70.0+46.0*row as f32,247.0,46.0]);}
        if p.get(1)?.starts_with("icon"){return Some([52.0,78.0+46.0*row as f32,28.0,28.0]);}
        let i=p.get(1)?.parse::<usize>().ok()?;if i<4{return Some([146.0+18.0*i as f32,79.0+46.0*row as f32,14.0,25.0]);}
    }None
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn panel_stays_inside_top_right(){let b=[-100.0,100.0,80.0,-32.5];let p=placement(b,component("sys_bayo_panel").unwrap()).unwrap();assert!(p.x>0.0&&p.x<100.0);assert!(p.y>20.0&&p.y<80.0);}
    #[test]fn anchor_tracks_camera_pan_and_zoom(){let a=placement([-100.0,100.0,80.0,-32.5],[0.0,0.0,291.0,210.0]).unwrap();let b=placement([-50.0,150.0,80.0,-32.5],[0.0,0.0,291.0,210.0]).unwrap();assert!((b.x-a.x-50.0).abs()<0.001);let c=placement([-200.0,200.0,160.0,-65.0],[0.0,0.0,291.0,210.0]).unwrap();assert!((c.scale_x-a.scale_x*2.0).abs()<0.001);}
    #[test]fn invalid_camera_bounds_are_rejected(){for b in [[0.0;4],[1.0,0.0,5.0,0.0],[f32::NAN,10.0,10.0,0.0]]{assert!(placement(b,[0.0;4]).is_none());}}
    #[test]fn pool_uses_shared_registered_emitters(){for row in 0..16{assert_eq!(effect_name(&format!("sys_bayo_row{}_3_d9",row)),"bayonetta_bayo_row0_3_d9");assert_eq!(effect_name(&format!("sys_bayo_row{}_witch",row)),"bayonetta_bayo_row0_witch");}assert_eq!(effect_name("sys_bayo_total_0_d3"),"bayonetta_bayo_total_0_d3");}
    #[test]fn chart_matches_timer_width_and_sits_below_it(){let p=placement([0.0,1280.0,720.0,0.0],component("sys_bayo_panel").unwrap()).unwrap();assert!((p.x-1148.0).abs()<0.01);let half_height=210.0*(200.0/291.0)/2.0;assert!((720.0-p.y-half_height-84.0).abs()<0.01);assert!((p.scale_x-10.0*(200.0/291.0)).abs()<0.01);}
}

// Screen-space coordinates: pixels on the centered 1280x720 overlay plane.
pub fn overlay(box_px:[f32;4],width_px:f32)->Option<Placement>{
    if !width_px.is_finite(){return None;}
    let width_px=width_px.max(160.0).min(240.0);let s=width_px/291.0;let [x,y,w,h]=box_px;
    Some(Placement{x:1280.0-32.0-width_px+(x+w/2.0)*s-640.0,y:360.0-84.0-(y+h/2.0)*s,scale_x:10.0*s,scale_y:10.0*s})
}
