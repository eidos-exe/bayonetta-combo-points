use crate::score::Category;
// Names are vanilla Smash motion names. Bullet collision attributes are checked
// separately; the held-button state is deliberately not a hit classification.
pub fn physical(motion:&str, frame:f32)->Option<Category> {
    use Category::*;
    Some(match motion {
        "attack_11"|"attack_12"|"attack_13"|"attack_hi3"|
        "attack_air_f"|"attack_air_f2"|"attack_air_f3"|"attack_100_end"|"attack_dash"|"catch_attack" => Punch,
        "attack_100"=>ContinuousPunch,
        "throw_f"=>Punch,
        "throw_b"|"throw_hi"|"throw_lw"=>Kick,
        "attack_air_n_hold"|"attack_air_hi_hold"=>ContinuousKick,
        "attack_s3_s"|"attack_s3_s2"|"attack_s3_s3"|"attack_lw3"|
        "attack_air_n"|"attack_air_b"|"attack_air_hi"|"attack_air_lw"|
        "landing_air_lw"|"attack_lw4"|"special_s"|"special_s_hold_end"|
        "special_air_s_u"|"special_air_s_d"|"special_air_s_d_hit"=>Kick,
        "attack_s4_s"|"attack_s4_hi"|"attack_s4_lw"|"attack_hi4"=>Punch,
        "special_hi"|"special_air_hi"=>if frame<15.0 {Punch}else{ContinuousPunch},
        // Bullet Climax is handled as an actual bullet, not an empty special.
        _=>return None,
    })
}
pub const MOTIONS:&[&str]=&["throw_f","throw_b","throw_hi","throw_lw","attack_11","attack_12","attack_13","attack_hi3","attack_air_f","attack_air_f2","attack_air_f3","attack_100_end","attack_100","attack_s3_s","attack_s3_s2","attack_s3_s3","attack_lw3","attack_air_n","attack_air_b","attack_air_hi","attack_air_lw","attack_air_n_hold","attack_air_hi_hold","attack_dash","catch_attack","landing_air_lw","attack_lw4","special_s","special_s_hold_end","special_air_s_u","special_air_s_d","special_air_s_d_hit","attack_s4_s","attack_s4_hi","attack_s4_lw","attack_hi4","special_hi","special_air_hi"];
pub fn family(physical_present:bool, bullets_present:bool, physical:Option<Category>)->Option<Category> {
    match (physical_present,bullets_present) {
        (false,true)=>Some(Category::Handgun),
        (true,false)|(false,false)=>physical,
        // Without a verified contact hitbox ID, guessing would mislabel hits.
        _=>None,
    }
}
pub fn mixed_contact(damage:f32,physical_power:f32,bullet_power:f32,physical:Option<Category>)->Option<Category>{
    if !damage.is_finite()||damage<=0.0{return None;}
    let distance=|power:f32|{
        if !power.is_finite()||power<=0.0{return f32::INFINITY;}
        [1.0,1.2].iter().map(|m|(damage-power*m).abs()/power).fold(f32::INFINITY,f32::min)
    };
    if distance(bullet_power)<distance(physical_power){Some(Category::Handgun)}else{physical}
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn witch_twist_has_two_native_categories(){assert_eq!(physical("special_hi",7.0),Some(Category::Punch));assert_eq!(physical("special_hi",16.0),Some(Category::ContinuousPunch));}
    #[test]fn side_special_phases_stay_kicks(){for m in ["special_s","special_s_hold_end","special_air_s_u","special_air_s_d"] {assert_eq!(physical(m,25.0),Some(Category::Kick));}}
    #[test]fn all_normal_attack_phases_have_a_category(){for m in MOTIONS{assert!(physical(m,20.0).is_some(),"Unmapped attack {}",m);}}
    #[test]fn extended_aerials_and_landing_are_covered(){assert_eq!(physical("attack_air_n_hold",20.0),Some(Category::ContinuousKick));assert_eq!(physical("attack_air_hi_hold",20.0),Some(Category::ContinuousKick));assert_eq!(physical("landing_air_lw",1.0),Some(Category::Kick));}
    #[test]fn simultaneous_shots_and_physical_hits_use_contact_damage(){for (damage,physical_power,expected) in [(0.5,1.4,Category::Handgun),(1.4,1.4,Category::Punch),(0.6,1.4,Category::Handgun),(1.68,1.4,Category::Punch),(0.2,0.2,Category::Punch),(0.5,0.2,Category::Handgun)]{assert_eq!(mixed_contact(damage,physical_power,0.5,Some(Category::Punch)),Some(expected));}}
    #[test]fn gunfire_is_not_inferred_from_move_or_button(){assert_eq!(family(false,true,Some(Category::Kick)),Some(Category::Handgun));assert_eq!(family(true,false,Some(Category::Kick)),Some(Category::Kick));assert_eq!(family(true,true,Some(Category::Kick)),None);assert_eq!(physical("special_lw",1.0),None);}
}
