// Native cockpit000.idd row templates; scalar and RGB tracks keep source values.
use crate::native_hud_tracks as native;

pub fn scalar(keys:&[(f32,f32)],age:f32,cubic:bool)->f32{
    if keys.is_empty(){return 1.0;}if age<=keys[0].0{return keys[0].1;}
    for i in 1..keys.len(){if age<keys[i].0{
        let t=(age-keys[i-1].0)/(keys[i].0-keys[i-1].0);let a=keys[i-1].1;let b=keys[i].1;
        if !cubic{return a+(b-a)*t;}
        let prev=keys[i.saturating_sub(2)].1;let next=keys[(i+1).min(keys.len()-1)].1;
        let t2=t*t;let t3=t2*t;
        return (2.0*t3-3.0*t2+1.0)*a+(t3-2.0*t2+t)*0.5*(b-prev)+(-2.0*t3+3.0*t2)*b+(t3-t2)*0.5*(next-a);
    }}keys.last().unwrap().1
}
pub fn rgb(keys:&[(f32,[f32;3])],age:f32)->[f32;3]{
    if keys.is_empty(){return [1.0;3];}let mut value=keys.last().unwrap().1;
    if age<=keys[0].0{value=keys[0].1;}else{for i in 1..keys.len(){if age<keys[i].0{let t=(age-keys[i-1].0)/(keys[i].0-keys[i-1].0);for j in 0..3{value[j]=keys[i-1].1[j]+(keys[i].1[j]-keys[i-1].1[j])*t;}break;}}}
    for x in value.iter_mut(){*x=(*x/255.0).max(0.0).min(1023.0/255.0);}value
}
pub fn entry(age:u32)->(f32,f32){
    let p=scalar(native::T900_PATH_PARAMETER,age as f32,false).min(2.0);let points=native::T900_PATH_X;
    let i=(p as usize).min(points.len()-2);let t=p-i as f32;
    let x=points[i]+(points[i+1]-points[i])*t;
    // The source row is 717 units wide; this adapter row is 247 canvas pixels.
    ((x-points[points.len()-1])*247.0/717.0,scalar(native::T900_ALPHA,age as f32,true).max(0.0).min(255.0)/255.0)
}
// Native row ordering approaches its target by at most 0.2 row units (e258e0/e2596c).
pub fn settle(current:f32,target:f32)->f32{let d=target-current;if d.abs()<=0.20001{target}else{current+if d>0.0{0.2}else{-0.2}}}
pub fn appearance(v:VisualRow)->(f32,f32,[f32;3]){
    if let Some(age)=v.leaving{return (scalar(native::T902_SCALE,age as f32,true),scalar(native::T902_ALPHA,age as f32,false).max(0.0).min(255.0)/255.0,rgb(native::T902_RGB,age as f32));}
    let gain=if v.row.witch_time{rgb(native::T903_RGB,v.age as f32)}else{[1.0;3]};
    let pulse=if v.age<10{rgb(native::TROW_BASE_RGB,v.age as f32)}else if !v.refreshed{[1.0;3]}else if v.row.witch_time{rgb(native::T905_RGB,v.flash as f32)}else{rgb(native::T904_RGB,v.flash as f32)};
    let mut color=[0.0;3];for i in 0..3{color[i]=(gain[i]*pulse[i]).min(1023.0/255.0);}
    let scale=if v.row.witch_time&&v.refreshed{scalar(native::T905_SCALE,v.flash as f32,false)}else{1.0};
    (scale,entry(v.age).1,color)
}
#[derive(Clone,Copy)]pub struct VisualRow{pub row:crate::score::Row,pub y:f32,pub target:f32,pub age:u32,pub flash:u32,pub refreshed:bool,pub leaving:Option<u32>}
pub struct Pool{pub rows:[Option<VisualRow>;16]}
impl Pool{
    pub const fn new()->Self{Self{rows:[None;16]}}
    pub fn reset(&mut self){self.rows=[None;16];}
    pub fn sync(&mut self,source:&[Option<crate::score::Row>;3]){
        for visual in self.rows.iter_mut().flatten(){
            if !source.iter().flatten().any(|r|r.id==visual.row.id)&&visual.leaving.is_none(){visual.leaving=Some(0);}
        }
        for(i,row)in source.iter().enumerate(){if let Some(row)=row{
            if let Some(v)=self.rows.iter_mut().flatten().find(|v|v.row.id==row.id){
                if v.row.points!=row.points{v.flash=0;v.refreshed=true;}
                v.row=*row;v.target=i as f32;v.leaving=None;
            }else{
                let slot=self.rows.iter().position(Option::is_none).unwrap_or_else(||self.rows.iter().enumerate().max_by_key(|(_,r)|r.map(|v|v.leaving.unwrap_or(0)).unwrap_or(0)).unwrap().0);
                self.rows[slot]=Some(VisualRow{row:*row,y:i as f32,target:i as f32,age:0,flash:0,refreshed:false,leaving:None});
            }
        }}
    }
    pub fn settle_opening(&mut self){for v in self.rows.iter_mut().flatten(){if v.leaving.is_none()&&v.age<5{v.age=5;v.y=v.target;}}}
    pub fn advance(&mut self){for slot in self.rows.iter_mut(){if let Some(v)=slot{
        v.age=v.age.saturating_add(1);v.flash=v.flash.saturating_add(1);v.y=settle(v.y,v.target);
        if let Some(age)=v.leaving{if age>=20{*slot=None;}else{v.leaving=Some(age+1);}}
    }}}
    pub fn active(&self)->bool{self.rows.iter().any(Option::is_some)}
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn native_entry_arrives_from_right_and_becomes_opaque(){let (x,a)=entry(0);assert!(x>0.0&&a==0.0);assert_eq!(entry(5),(0.0,1.0));assert_eq!(entry(200),(0.0,1.0));}
    #[test]fn older_rows_settle_without_overshooting(){let mut x=2.0;for _ in 0..30{x=settle(x,1.0);assert!(x>=1.0);}assert_eq!(x,1.0);}
    #[test]fn removal_keeps_an_exit_object_without_replacing_surviving_rows(){let mut c=crate::score::Chain::new();c.hit(crate::score::Category::Punch,false,false);let mut p=Pool::new();p.sync(&c.rows);let id=c.rows[0].unwrap().id;c.clear();p.sync(&c.rows);assert!(p.active());assert_eq!(p.rows[0].unwrap().row.id,id);for _ in 0..21{p.advance();}assert!(!p.active());}
    #[test]fn repeated_gunfire_refreshes_one_row_instead_of_spawning_duplicates(){let mut c=crate::score::Chain::new();let mut p=Pool::new();for _ in 0..30{c.hit(crate::score::Category::Handgun,false,false);p.sync(&c.rows);p.advance();}assert_eq!(p.rows.iter().flatten().count(),1);assert_eq!(p.rows[0].unwrap().row.count,30);}
    #[test]fn native_refresh_brightness_peaks_then_settles(){let a=rgb(native::T904_RGB,5.0);assert!((a[0]-555.0/255.0).abs()<0.00001);assert_eq!(rgb(native::T904_RGB,30.0),[1.0;3]);}
    #[test]fn native_departure_is_invisible_by_frame_seventeen(){assert!(scalar(native::T902_ALPHA,17.0,false)<=0.0);assert_eq!(scalar(native::T902_ALPHA,30.0,false),0.0);}
    #[test]fn entry_does_not_restart_an_unrequested_refresh(){let mut c=crate::score::Chain::new();c.hit(crate::score::Category::Punch,false,false);let mut p=Pool::new();p.sync(&c.rows);for _ in 0..11{p.advance();}let (_,a,rgb)=appearance(p.rows[0].unwrap());assert_eq!(a,1.0);assert_eq!(rgb,[1.0;3]);}
}

// A fresh chain starts settled. Only an increase within a live chain pulses.
pub fn multiplier_start_age(previous:u32)->u32{if previous==0{50}else{0}}
#[cfg(test)]mod header_entry_tests{
 #[test]fn first_header_is_settled_and_later_multiplier_changes_pulse(){assert_eq!(super::multiplier_start_age(0),50);assert_eq!(super::multiplier_start_age(10),0);assert_eq!(super::multiplier_start_age(0),50);}
}

#[cfg(test)]mod opening_tests{
 use super::*;use crate::score::{Chain,Category};
 #[test]fn first_snapshot_is_visible_in_place_but_following_rows_still_slide(){let mut c=Chain::new();let mut p=Pool::new();c.hit(Category::Punch,false,false);p.sync(&c.rows);p.settle_opening();let first=p.rows[0].unwrap();assert_eq!(entry(first.age),(0.0,1.0));assert_eq!(appearance(first).1,1.0);c.hit(Category::Kick,false,false);p.sync(&c.rows);let second=p.rows.iter().flatten().find(|v|v.row.category==Category::Kick).unwrap();assert_eq!(second.age,0);assert!(entry(second.age).0>0.0);assert_eq!(entry(first.age).0,0.0);}
 #[test]fn opening_does_not_revive_or_change_retiring_rows(){let mut c=Chain::new();let mut p=Pool::new();c.hit(Category::Punch,false,false);p.sync(&c.rows);c.clear();p.sync(&c.rows);p.settle_opening();let v=p.rows[0].unwrap();assert_eq!(v.age,0);assert_eq!(v.leaving,Some(0));}
}
