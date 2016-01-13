//! Entity component system.
//!
//! Using TypeIDs, no macros.
//! Heavily influenced by https://github.com/andybarron/rustic-ecs
//! but should be more memory efficient (we use one box per component only)
//!
//! License: Apache2.0/MIT

use std::collections::{HashSet, HashMap};
use std::collections::hash_map::{Iter as HIter, IterMut as HIterMut}; 
use std::any::{TypeId, Any};
use std::hash::Hash;
use std::fmt;
use std::fmt::Debug;

trait CompList<EntityId> : Any {
    fn remove(&mut self, &EntityId) -> bool;
    fn as_any_mut(&mut self) -> &mut Any;
    fn as_any(&self) -> &Any;
    fn debug(&self, &mut fmt::Formatter, &EntityId, &str) -> Option<fmt::Result>;
}

impl<C: Any, EntityId: Hash + Eq + Any> CompList<EntityId> for CList<C, EntityId> {
    fn remove(&mut self, a: &EntityId) -> bool { self.0.remove(a).is_some() }
    fn as_any_mut(&mut self) -> &mut Any { self }
    fn as_any(&self) -> &Any { self }
    fn debug(&self, f: &mut fmt::Formatter, a: &EntityId, prefix: &str) -> Option<fmt::Result> {
        self.1.as_ref().and_then(|b| self.0.get(a).map(|c| {
            try!(f.write_str(prefix));
            b(c, f)
        }))
    }
}

struct CList<C: Any, EntityId: Hash + Eq>(HashMap<EntityId, C>, Option<Box<Fn(&C, &mut fmt::Formatter) -> fmt::Result>>);

impl<C: Any, EntityId: Hash + Eq> CList<C, EntityId> {
    fn new_nodbg() -> CList<C, EntityId> { CList(HashMap::new(), None) }
}

impl<C: Any + Debug, EntityId: Hash + Eq> CList<C, EntityId> {
    fn new_dbg() -> CList<C, EntityId> { CList(HashMap::new(), Some(Box::new(
        |c: &C, f: &mut fmt::Formatter| (c as &Debug).fmt(f)))) }
}

pub struct Iter<'a, EntityId: 'a, C: 'a>(Option<HIter<'a, EntityId, C>>);

impl<'a, EntityId: 'a, C: 'a> Iterator for Iter<'a, EntityId, C> {
    type Item = (&'a EntityId, &'a C);
    fn next(&mut self) -> Option<(&'a EntityId, &'a C)> { self.0.as_mut().and_then(|s| s.next()) }
}

pub struct IterMut<'a, EntityId: 'a, C: 'a>(Option<HIterMut<'a, EntityId, C>>);

impl<'a, EntityId: 'a, C: 'a> Iterator for IterMut<'a, EntityId, C> {
    type Item = (&'a EntityId, &'a mut C);
    fn next(&mut self) -> Option<(&'a EntityId, &'a mut C)> { self.0.as_mut().and_then(|s| s.next()) }
}

#[derive(Default)]
/// Entity-Component map, implemented as a double HashMap,
/// first over component TypeId, then over EntityId.
pub struct ECMap<EntityId: Hash + Eq = u32> {
    // Index to easier iterate over all entities
    entities: HashSet<EntityId>,
    // The hashmap's value is a CList<C, EntityId> where C has the same TypeId.
    components: HashMap<TypeId, Box<CompList<EntityId>>>,
    last_entity: EntityId,
}

impl<EntityId: Hash + Copy + Eq + Any> ECMap<EntityId> {

    fn clist<C: Any>(&self) -> Option<&CList<C, EntityId>> {
        self.components.get(&TypeId::of::<C>()).map(|c|
            c.as_any().downcast_ref::<CList<C, EntityId>>().unwrap())
    }

    fn clist_mut<C: Any>(&mut self) -> Option<&mut CList<C, EntityId>> {
        self.components.get_mut(&TypeId::of::<C>()).map(|c|
            c.as_any_mut().downcast_mut::<CList<C, EntityId>>().unwrap())
    }

    /// Inserts or replaces an entity component. The old component, if there
    /// was one, is returned.
    /// If the entity or component did not at all exist, it is also added.
    pub fn insert<C: Any>(&mut self, e: EntityId, c: C) -> Option<C> {
        self.entities.insert(e);
        let q = self.components.entry(TypeId::of::<C>())
            .or_insert_with(|| Box::new(CList::<C, EntityId>::new_nodbg()));
        q.as_any_mut().downcast_mut::<CList<C, EntityId>>().unwrap().0.insert(e, c)
    }

    /// Inserts a component type and enables debugging for that component.
    /// In case the component type already exists, nothing is changed and false is returned.  
    pub fn insert_component<C: Any + Debug>(&mut self) -> bool {
        if self.contains_component::<C>() { return false }
        self.components.insert(TypeId::of::<C>(), Box::new(CList::<C, EntityId>::new_dbg()));
        true
    }

    /// Returns a reference to an entity component, or None if it does not exist.
    pub fn get<C: Any>(&self, e: EntityId) -> Option<&C> {
        self.clist::<C>().and_then(|s| s.0.get(&e))
    }

    /// Returns a mutable reference to an entity component, or None if it does not exist.
    pub fn get_mut<C: Any>(&mut self, e: EntityId) -> Option<&mut C> {
        self.clist_mut::<C>().and_then(|s| s.0.get_mut(&e))
    }

    /// Check whether an entity component exists.
    pub fn contains<C: Any>(&self, e: EntityId) -> bool {
        self.get::<C>(e).is_some()
    }

    /// Check whether an entity exists.
    pub fn contains_entity(&self, e: EntityId) -> bool {
        self.entities.contains(&e)
    }

    /// Check whether a component exists.
    pub fn contains_component<C: Any>(&self) -> bool {
        self.components.contains_key(&TypeId::of::<C>())
    }

    /// Removes an entity component.
    pub fn remove<C: Any>(&mut self, e: EntityId) -> bool {
        self.components.get_mut(&TypeId::of::<C>()).map(
            |s| s.remove(&e)).unwrap_or(false)
    }

    /// Remove an entity (existing components will no longer have that entity).
    pub fn remove_entity(&mut self, e: EntityId) -> bool {
        if !self.entities.remove(&e) { return false };
        for (_, c) in self.components.iter_mut() { c.remove(&e); }
        true
    }

    /// Remove a component (existing entities will no longer have that component).
    pub fn remove_component<C: Any>(&mut self) -> bool {
        self.components.remove(&TypeId::of::<C>()).is_some()
    }

    /// Clones the component for all entities and returns a Vec of those.
    pub fn clone_with<C: Any + Clone>(&self) -> Vec<(EntityId, C)> {
        self.iter_with::<C>().map(|(k, v)| (*k, v.clone())).collect() 
    }

    /// Iterates over all entities having a certain component.
    pub fn iter_with<C: Any>(&self) -> Iter<EntityId, C> {
        let c = if let Some(s) = self.clist::<C>() { s } else { return Iter(None) };
        Iter(Some(c.0.iter()))
    }

    /// Iterates over all entities having a certain component, yielding mutable references.
    pub fn iter_mut_with<C: Any>(&mut self) -> IterMut<EntityId, C> {
        let c = if let Some(s) = self.clist_mut::<C>() { s } else { return IterMut(None) };
        IterMut(Some(c.0.iter_mut()))
    }
}

impl<EntityId: Hash + Eq + Debug> Debug for ECMap<EntityId> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // We cannot use debug_map / debug_set here, unfortunately.
        try!(f.write_str("ECMap {"));
        let mut fent = true;
        for e in self.entities.iter() {
            if !fent { try!(f.write_str(",\n")) };
            fent = false;
            try!(write!(f, "{:?}: (", e));
            let mut fcomp = true;
            for c in self.components.values() {
                match c.debug(f, e, if fcomp {""} else {", "}) {
                    Some(Err(fe)) => return Err(fe),
                    Some(_) => fcomp = false,
                    None => {},
                }
            }
            try!(f.write_str(")"));
        } 
        f.write_str("}")
    }
}

impl ECMap<u32> {
    /// Returns a new ECMap with u32 as EntityId.
    pub fn new() -> ECMap<u32> { Default::default() }

    /// Generates an EntityId and inserts it.
    pub fn insert_entity(&mut self) -> u32 {
        self.last_entity += 1;
        self.entities.insert(self.last_entity);
        self.last_entity
    }
}

impl ECMap<usize> {
    /// Returns a new ECMap with usize as EntityId.
    pub fn new_usize() -> ECMap<usize> { Default::default() }

    /// Generates an EntityId and inserts it.
    pub fn insert_entity(&mut self) -> usize {
        self.last_entity += 1;
        self.entities.insert(self.last_entity);
        self.last_entity
    }
}

impl ECMap<u64> {
    /// Returns a new ECMap with u64 as EntityId.
    pub fn new_u64() -> ECMap<u64> { Default::default() }

    /// Generates an EntityId and inserts it.
    pub fn insert_entity(&mut self) -> u64 {
        self.last_entity += 1;
        self.entities.insert(self.last_entity);
        self.last_entity
    }
}

#[test]
fn do_test() {

    let mut e = ECMap::new();
    let id = e.insert_entity();
    assert_eq!(e.insert(id, 6u16), None);
    assert_eq!(e.insert(id, 10u32), None);
    assert_eq!(e.insert(id, 7u16), Some(6u16));
    { 
        let q: &mut u32 = e.get_mut(id).unwrap();
         *q += 1;
    }
    assert_eq!(e.get::<u32>(id), Some(&11u32));
    assert_eq!(e.get::<i32>(id), None);
    assert_eq!(&*e.clone_with::<u16>(), &[(id, 7u16)][..]);
    e.remove_entity(id);
    assert_eq!(&*e.clone_with::<u16>(), &[][..]);
}

#[test]
fn debug_test() {

    let mut e = ECMap::new();

    #[derive(Debug)]
    struct Name(&'static str);

    e.insert_component::<Name>();
    e.insert_component::<u32>();

    let id1 = e.insert_entity();
    e.insert(id1, Name("Test"));
    e.insert(id1, 7u32);

    let id2 = e.insert_entity();
    e.insert(id2, 5u16);

    // Order is not guaranteed when iterating a hashmap.
    let s1 = "ECMap {1: (7, Name(\"Test\")),\n2: ()}";
    let s2 = "ECMap {2: (),\n1: (7, Name(\"Test\"))}";
    let s3 = "ECMap {1: (Name(\"Test\"), 7),\n2: ()}";
    let s4 = "ECMap {2: (),\n1: (Name(\"Test\"), 7)}";

    let s = format!("{:?}", e);
    println!("{}", s);
    assert!(s == s1 || s == s2 || s == s3 || s == s4);
}
