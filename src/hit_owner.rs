#[derive(Clone, Copy)]
pub enum Source {
    Fighter,
    OwnedBy { owner: u32, item: bool },
}
#[derive(Debug, PartialEq)]
pub struct Owner {
    pub fighter: u32,
    pub via_item: bool,
}
/// Resolve the live holder or current thrower, never an item's original creator.
pub fn fighter_owner(mut id: u32, mut source: impl FnMut(u32) -> Option<Source>) -> Option<Owner> {
    let mut visited = [u32::MAX; 4];
    let mut via_item = false;
    for depth in 0..visited.len() {
        if visited[..depth].contains(&id) {
            return None;
        }
        visited[depth] = id;
        match source(id)? {
            Source::Fighter => {
                return Some(Owner {
                    fighter: id,
                    via_item,
                })
            }
            Source::OwnedBy { owner, item } => {
                id = owner;
                via_item |= item;
            }
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_fighter_and_articles_keep_their_owner() {
        for id in [1, 2] {
            assert_eq!(
                fighter_owner(id, |i| Some(if i == 1 {
                    Source::Fighter
                } else {
                    Source::OwnedBy {
                        owner: 1,
                        item: false,
                    }
                })),
                Some(Owner {
                    fighter: 1,
                    via_item: false
                })
            );
        }
    }
    #[test]
    fn held_and_thrown_items_follow_current_owner() {
        for holder in [1, 7] {
            assert_eq!(
                fighter_owner(20, |id| match id {
                    20 => Some(Source::OwnedBy {
                        owner: holder,
                        item: true
                    }),
                    i if i == holder => Some(Source::Fighter),
                    _ => None,
                }),
                Some(Owner {
                    fighter: holder,
                    via_item: true
                })
            );
        }
    }
    #[test]
    fn shot_owned_by_an_item_retains_item_origin() {
        assert_eq!(
            fighter_owner(30, |id| match id {
                30 => Some(Source::OwnedBy {
                    owner: 20,
                    item: false
                }),
                20 => Some(Source::OwnedBy {
                    owner: 7,
                    item: true
                }),
                7 => Some(Source::Fighter),
                _ => None,
            }),
            Some(Owner {
                fighter: 7,
                via_item: true
            })
        );
    }
    #[test]
    fn inactive_unowned_cyclic_and_deep_chains_are_rejected() {
        assert_eq!(fighter_owner(20, |_| None), None);
        assert_eq!(
            fighter_owner(20, |_| Some(Source::OwnedBy {
                owner: 20,
                item: true
            })),
            None
        );
        assert_eq!(
            fighter_owner(20, |id| Some(Source::OwnedBy {
                owner: if id == 20 { 21 } else { 20 },
                item: true
            })),
            None
        );
        assert_eq!(
            fighter_owner(20, |id| Some(Source::OwnedBy {
                owner: id + 1,
                item: true
            })),
            None
        );
    }
}
