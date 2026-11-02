use crate::score::Category;
#[derive(Clone,Copy,Debug)]
pub struct Contact {pub source:usize,pub source_object:u32,pub category:Category,age:u8}
impl Contact {pub fn new(source:usize,source_object:u32,category:Category)->Self{Self{source,source_object,category,age:0}}}
#[derive(Clone,Copy)]
pub struct Damage {object:usize,percent:f32,ready:bool,pending:Option<Contact>,unclaimed:Option<(u16,u8)>}
impl Damage {
    pub const fn new()->Self{Self{object:0,percent:0.0,ready:false,pending:None,unclaimed:None}}
    pub fn observe(&mut self,object:usize,percent:f32,reset:bool,mask:u16,received_callback:bool)->Option<Contact>{
        let same=self.ready&&self.object==object&&!reset&&percent.is_finite();
        let increase=same&&percent>self.percent+0.0001;
        if !same{self.pending=None;self.unclaimed=None;}
        self.object=object;self.percent=percent;self.ready=!reset&&percent.is_finite();
        if !increase{return None;}
        let contact=self.pending.take().filter(|c|mask==1u16<<c.source);
        if contact.is_none()&&received_callback{self.unclaimed=Some((mask,0));}
        contact
    }
    pub fn register(&mut self,contact:Contact)->Option<Contact>{
        // Some engine paths notify the collision after received-damage runs.
        // Match only an unclaimed callback from the same frame and attacker.
        if self.unclaimed.map(|(mask,age)|age==0&&mask==1u16<<contact.source).unwrap_or(false){
            self.unclaimed=None;return Some(contact);
        }
        self.pending=Some(contact);None
    }
    pub fn tick(&mut self){
        if let Some(c)=self.pending.as_mut(){c.age=c.age.saturating_add(1);if c.age>2{self.pending=None;}}
        if let Some((_,age))=self.unclaimed.as_mut(){*age=age.saturating_add(1);if *age>0{self.unclaimed=None;}}
    }
}
#[cfg(test)]mod tests{
    use super::*;
    fn contact()->Contact{Contact::new(1,9,Category::Punch)}
    #[test]fn no_points_for_shield_or_whiff(){let mut d=Damage::new();d.observe(1,0.0,false,2,false);d.register(contact());assert!(d.observe(1,0.0,false,2,true).is_none());for _ in 0..3{d.tick();}assert!(d.observe(1,10.0,false,2,false).is_none());}
    #[test]fn immediate_or_delayed_percent_scores_once(){let mut d=Damage::new();d.observe(1,0.0,false,2,false);d.register(contact());assert!(d.observe(1,0.0,false,2,true).is_none());assert!(d.observe(1,5.0,false,2,false).is_some());assert!(d.observe(1,5.0,false,2,false).is_none());}
    #[test]fn damage_before_collision_still_matches(){let mut d=Damage::new();d.observe(1,0.0,false,2,false);assert!(d.observe(1,5.0,false,2,true).is_none());assert!(d.register(contact()).is_some());assert!(d.register(contact()).is_none());}
    #[test]fn wrong_attacker_is_not_credited(){let mut d=Damage::new();d.observe(1,0.0,false,2,false);d.register(contact());assert!(d.observe(1,5.0,false,4,true).is_none());assert!(d.register(contact()).is_none());}
    #[test]fn reset_cannot_carry_a_pending_hit(){let mut d=Damage::new();d.observe(1,0.0,false,2,false);d.register(contact());d.observe(1,0.0,true,2,false);assert!(d.observe(2,5.0,false,2,true).is_none());}
    #[test]fn post_damage_notification_registers_before_sampling(){let mut d=Damage::new();d.observe(1,0.0,false,2,false);d.register(contact());let c=d.observe(1,5.0,false,2,false).expect("Notification must credit damage already applied by the engine");assert_eq!(c.source,1);assert_eq!(c.category,Category::Punch);assert!(d.observe(1,5.0,false,2,false).is_none());}
}
