#[derive(Clone,Copy)]pub struct Candidate{pub slot:usize,pub controller:Option<i32>,pub cpu:bool,pub bayonetta:bool}
// Select before scoring, so an opponent cannot claim an idle chart by hitting first.
pub fn select(candidates:&[Candidate])->Option<usize>{candidates.iter().filter(|c|c.bayonetta&&!c.cpu&&c.controller.map(|i|(0..=9).contains(&i)).unwrap_or(false)).min_by_key(|c|(c.controller.unwrap(),c.slot)).map(|c|c.slot)}
#[cfg(test)]mod tests{
use super::*;
#[test]fn remote_and_cpu_bayonettas_cannot_claim_chart(){assert_eq!(select(&[Candidate{slot:0,controller:None,cpu:false,bayonetta:true},Candidate{slot:1,controller:Some(0),cpu:true,bayonetta:true},Candidate{slot:5,controller:Some(2),cpu:false,bayonetta:true}]),Some(5));}
#[test]fn selection_follows_controller_order_not_fighter_slot(){assert_eq!(select(&[Candidate{slot:0,controller:Some(3),cpu:false,bayonetta:true},Candidate{slot:6,controller:Some(0),cpu:false,bayonetta:true}]),Some(6));}
#[test]fn local_non_bayonetta_does_not_display_remote_chart(){assert_eq!(select(&[Candidate{slot:0,controller:Some(0),cpu:false,bayonetta:false},Candidate{slot:1,controller:None,cpu:false,bayonetta:true}]),None);}
#[test]fn invalid_controller_indices_are_rejected(){assert_eq!(select(&[Candidate{slot:0,controller:Some(-1),cpu:false,bayonetta:true},Candidate{slot:1,controller:Some(10),cpu:false,bayonetta:true}]),None);}
}
