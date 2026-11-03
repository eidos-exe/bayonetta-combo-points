// Source attack scoring is adapted from Bayonetta; multiplier steps are tuned for Smash.
// Row creation: 0x38e18 / 0x3a9f0. Row refresh: 0x3ae3c.
// Point calculation: 0x36660. Multiplier: 0x39e78. Settlement: 0x37b00.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Category { Punch, Kick, ContinuousPunch, ContinuousKick, Handgun }
impl Category {
    pub fn index(self)->usize {self as usize}
    pub fn icon(self)->u8 {[0,1,7,8,11][self.index()]}
    pub fn grouped(self)->bool {matches!(self,Self::ContinuousPunch|Self::ContinuousKick|Self::Handgun)}
    pub fn base(self)->u32 {[10,10,2,2,1][self.index()]}
}
pub const DEFAULT_MULTIPLIER_STEP:u32=25;
pub const NEW_ROW_TICKS:u32=180;
pub const GROUP_REFRESH_TICKS:u32=60;
#[derive(Clone,Copy,Debug)]
pub struct Row {pub category:Category,pub points:u32,pub count:u32,pub remaining:u32,pub witch_time:bool,pub id:u32,pub pulse:u32}
#[derive(Clone,Copy)]
pub struct Chain {
    pub points:u32,pub remaining:u32,pub rows:[Option<Row>;3],normal_count:u32,multiplier_step:u32,defense_held:bool,
    pub revision:u32,pub last_settlement:u64,pub total_settled:u64,pub settlement_id:u32,
}
impl Chain {
    pub const fn new()->Self {Self{points:0,remaining:0,rows:[None;3],normal_count:0,multiplier_step:DEFAULT_MULTIPLIER_STEP,defense_held:false,revision:0,last_settlement:0,total_settled:0,settlement_id:0}}
    pub fn active(&self)->bool {self.rows.iter().any(Option::is_some)}
    pub fn clear(&mut self){let rev=self.revision.wrapping_add(1);let last=self.last_settlement;let total=self.total_settled;let settlement=self.settlement_id;let step=self.multiplier_step;*self=Self::new();self.multiplier_step=step;self.revision=rev;self.last_settlement=last;self.total_settled=total;self.settlement_id=settlement;}
    fn settle(&mut self){self.settlement_id=self.settlement_id.wrapping_add(1);let tenths=self.multiplier_uncapped_tenths();self.last_settlement=(self.points as u64).saturating_mul(tenths)/10;self.total_settled=self.total_settled.saturating_add(self.last_settlement);self.clear();}
    pub fn configure_multiplier(&mut self,step:u32){self.multiplier_step=step.max(1);}
    pub fn tick_defense(&mut self,paused:bool,witch_time:bool,bat_within:bool){
        if !self.active(){self.defense_held=false;return;}
        if witch_time||bat_within{self.defense_held=true;return;}
        if paused{return;}
        if self.defense_held{self.defense_held=false;for row in self.rows.iter_mut().flatten(){row.remaining=NEW_ROW_TICKS;}self.remaining=NEW_ROW_TICKS;return;}
        self.tick(false);
    }
    pub fn tick(&mut self,paused:bool){
        if paused||!self.active(){return;}
        let mut expired=false;
        for r in self.rows.iter_mut(){if let Some(row)=r{row.pulse=row.pulse.saturating_add(1);row.remaining=row.remaining.saturating_sub(1);if row.remaining==0{*r=None;expired=true;}}}
        if expired{let mut compact=[None;3];let mut n=0;for row in self.rows.iter().flatten(){compact[n]=Some(*row);n+=1;}self.rows=compact;self.revision=self.revision.wrapping_add(1);}
        self.remaining=self.rows.iter().flatten().map(|r|r.remaining).max().unwrap_or(0);
        if !self.active(){self.settle();}
    }
    pub fn hit(&mut self,category:Category,weave:bool,witch_time:bool)->u32{
        if !self.active(){self.clear();}
        if weave {self.normal_count=0;}else if !category.grouped(){self.normal_count=self.normal_count.saturating_add(1);}
        let mut base=category.base();
        // Native +50% branch precedes the ordinary punch/kick repeat penalty.
        if witch_time {base=base.saturating_add(base/2);}
        let earned=if category.grouped(){base}else{base.saturating_sub(self.normal_count.saturating_sub(5)*2).max(1)};
        if category.grouped(){if let Some(row)=self.rows.iter_mut().flatten().find(|r|r.category==category&&r.witch_time==witch_time){
            row.remaining=GROUP_REFRESH_TICKS;row.pulse=0;
            if row.count>=99 {self.remaining=self.rows.iter().flatten().map(|r|r.remaining).max().unwrap_or(0);return 0;}
            row.count+=1;row.points=row.points.saturating_add(earned);self.points=self.points.saturating_add(earned);
            self.remaining=self.rows.iter().flatten().map(|r|r.remaining).max().unwrap_or(0);self.revision=self.revision.wrapping_add(1);return earned;
        }}
        let row=Some(Row{category,points:earned,count:1,remaining:NEW_ROW_TICKS,witch_time,id:self.revision.wrapping_add(1),pulse:0});
        if let Some(empty)=self.rows.iter_mut().find(|r|r.is_none()){*empty=row;}else{self.rows[0]=self.rows[1];self.rows[1]=self.rows[2];self.rows[2]=row;}
        self.points=self.points.saturating_add(earned);self.remaining=NEW_ROW_TICKS;self.revision=self.revision.wrapping_add(1);earned
    }
    pub fn multiplier_uncapped_tenths(&self)->u64{10+self.points as u64/self.multiplier_step.max(1)as u64}
    pub fn multiplier_tenths(&self)->u32{self.multiplier_uncapped_tenths().min(99) as u32}
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn native_new_row_lifetime(){let mut c=Chain::new();c.hit(Category::Punch,false,false);for _ in 0..179{c.tick(false);}assert!(c.active());c.tick(false);assert!(!c.active());assert_eq!(c.last_settlement,10);}
    #[test]fn gunfire_groups_and_uses_native_refresh(){let mut c=Chain::new();c.hit(Category::Handgun,false,false);c.hit(Category::Handgun,false,false);assert_eq!(c.rows.iter().flatten().count(),1);assert_eq!(c.rows[0].unwrap().count,2);assert_eq!(c.rows[0].unwrap().remaining,60);assert_eq!(c.points,2);}
    #[test]fn grouped_count_caps_at_99(){let mut c=Chain::new();for _ in 0..130{c.hit(Category::Handgun,false,false);}assert_eq!(c.points,99);assert_eq!(c.rows[0].unwrap().count,99);}
    #[test]fn row_expiry_is_independent(){let mut c=Chain::new();c.hit(Category::Punch,false,false);for _ in 0..30{c.tick(false);}c.hit(Category::Kick,false,false);for _ in 0..150{c.tick(false);}assert_eq!(c.rows.iter().flatten().count(),1);assert_eq!(c.rows[0].unwrap().category,Category::Kick);}
    #[test]fn oldest_row_is_top(){let mut c=Chain::new();for cat in [Category::Punch,Category::Kick,Category::ContinuousPunch,Category::Handgun]{c.hit(cat,false,false);}assert_eq!(c.rows[0].unwrap().category,Category::Kick);assert_eq!(c.rows[2].unwrap().category,Category::Handgun);}
    #[test]fn native_normal_counter_is_shared(){let mut c=Chain::new();for cat in [Category::Punch,Category::Kick,Category::Punch,Category::Kick,Category::Punch]{assert_eq!(c.hit(cat,false,false),10);}assert_eq!(c.hit(Category::Kick,false,false),8);assert_eq!(c.hit(Category::Punch,true,false),10);assert_eq!(c.hit(Category::Punch,false,false),10);}
    #[test]fn witch_time_points_follow_integer_half_bonus(){let mut c=Chain::new();assert_eq!(c.hit(Category::Punch,false,true),15);assert_eq!(c.hit(Category::ContinuousPunch,false,true),3);assert_eq!(c.hit(Category::Handgun,false,true),1);}
    #[test]fn multiplier_and_settlement_are_not_display_capped(){let mut c=Chain::new();c.hit(Category::Punch,false,false);c.points=3000;assert_eq!(c.multiplier_tenths(),99);assert_eq!(c.multiplier_uncapped_tenths(),130);for _ in 0..180{c.tick(false);}assert_eq!(c.last_settlement,39000);}
    #[test]fn paused_native_clock_does_not_expire(){let mut c=Chain::new();c.hit(Category::Kick,false,false);for _ in 0..500{c.tick(true);}assert_eq!(c.remaining,180);}
}

#[cfg(test)]mod tuning_tests{
 use super::*;
 #[test]fn agreed_multiplier_steps(){let mut c=Chain::new();for(p,m)in[(0,10),(24,10),(25,11),(49,11),(50,12),(75,13),(100,14),(125,15),(150,16),(175,17)]{c.points=p;assert_eq!(c.multiplier_tenths(),m);}}
 #[test]fn witch_time_preserves_chain_then_restores_window(){let mut c=Chain::new();c.hit(Category::Handgun,false,false);for _ in 0..59{c.tick(false);}for _ in 0..400{c.tick_defense(false,true,false);}assert!(c.active());assert_eq!(c.points,1);assert_eq!(c.remaining,121);c.tick_defense(false,false,false);assert_eq!(c.remaining,180);for _ in 0..179{c.tick_defense(false,false,false);}assert!(c.active());c.tick_defense(false,false,false);assert!(!c.active());}
 #[test]fn witch_time_does_not_revive_settled_chain(){let mut c=Chain::new();c.hit(Category::Punch,false,false);for _ in 0..180{c.tick(false);}let id=c.settlement_id;c.tick_defense(false,true,false);c.tick_defense(false,false,false);assert!(!c.active());assert_eq!(c.settlement_id,id);}
 #[test]fn missed_counter_does_not_extend_chain(){let mut c=Chain::new();c.hit(Category::Punch,false,false);for _ in 0..180{c.tick_defense(false,false,false);}assert!(!c.active());}
 #[test]fn configured_interval_survives_chain_reset(){let mut c=Chain::new();c.configure_multiplier(50);c.clear();c.points=100;assert_eq!(c.multiplier_tenths(),12);}
}

#[cfg(test)]mod bat_within_tests{
 use super::*;
 #[test]fn successful_bats_preserve_points_and_restore_full_window(){let mut c=Chain::new();c.hit(Category::Punch,false,false);for _ in 0..175{c.tick(false);}for _ in 0..300{c.tick_defense(false,false,true);}assert_eq!(c.remaining,5);assert_eq!(c.points,10);c.tick_defense(false,false,false);assert_eq!(c.remaining,NEW_ROW_TICKS);assert_eq!(c.hit(Category::Kick,false,false),10);assert_eq!(c.points,20);}
 #[test]fn bats_do_not_start_or_revive_a_chain(){let mut c=Chain::new();c.tick_defense(false,false,true);c.tick_defense(false,false,false);assert!(!c.active());c.hit(Category::Punch,false,false);for _ in 0..180{c.tick(false);}let settled=c.settlement_id;c.tick_defense(false,false,true);c.tick_defense(false,false,false);assert!(!c.active());assert_eq!(c.settlement_id,settled);}
 #[test]fn ordinary_dodge_still_expires(){let mut c=Chain::new();c.hit(Category::Punch,false,false);for _ in 0..180{c.tick_defense(false,false,false);}assert!(!c.active());}
 #[test]fn overlapping_defenses_restore_only_after_both_end(){let mut c=Chain::new();c.hit(Category::Kick,false,false);c.tick(false);c.tick_defense(false,true,false);c.tick_defense(false,true,true);c.tick_defense(false,false,true);assert_eq!(c.remaining,179);c.tick_defense(false,false,false);assert_eq!(c.remaining,180);c.tick_defense(false,false,false);assert_eq!(c.remaining,179);}
}
