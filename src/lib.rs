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

trait CompList<E> : Any {
    fn remove(&mut self, &E) -> bool;
    fn as_any_mut(&mut self) -> &mut Any;
    fn as_any(&self) -> &Any;
    fn debug(&self, &mut fmt::Formatter, &E, &str) -> Option<fmt::Result>;
}

impl<C: Any, E: Hash + Eq + Any> CompList<E> for CList<C, E> {
    fn remove(&mut self, a: &E) -> bool { self.0.remove(a).is_some() }
    fn as_any_mut(&mut self) -> &mut Any { self }
    fn as_any(&self) -> &Any { self }
    fn debug(&self, f: &mut fmt::Formatter, a: &E, prefix: &str) -> Option<fmt::Result> {
        self.1.as_ref().and_then(|b| self.0.get(a).map(|c| {
            try!(f.write_str(prefix));
            b(c, f)
        }))
    }
}

struct CList<C: Any, E: Hash + Eq>(HashMap<E, C>, Option<Box<Fn(&C, &mut fmt::Formatter) -> fmt::Result>>);

impl<C: Any, E: Hash + Eq> CList<C, E> {
    fn new_nodbg() -> CList<C, E> { CList(HashMap::new(), None) }
}

impl<C: Any + Debug, E: Hash + Eq> CList<C, E> {
    fn new_dbg() -> CList<C, E> { CList(HashMap::new(), Some(Box::new(
        |c: &C, f: &mut fmt::Formatter| (c as &Debug).fmt(f)))) }
}

pub struct Iter<'a, E: 'a, C: 'a>(Option<HIter<'a, E, C>>);

impl<'a, E: 'a, C: 'a> Iterator for Iter<'a, E, C> {
    type Item = (&'a E, &'a C);
    fn next(&mut self) -> Option<(&'a E, &'a C)> { self.0.as_mut().and_then(|s| s.next()) }
}

pub struct IterMut<'a, E: 'a, C: 'a>(Option<HIterMut<'a, E, C>>);

impl<'a, E: 'a, C: 'a> Iterator for IterMut<'a, E, C> {
    type Item = (&'a E, &'a mut C);
    fn next(&mut self) -> Option<(&'a E, &'a mut C)> { self.0.as_mut().and_then(|s| s.next()) }
}

#[derive(Default)]
/// Entity-Component map, implemented as a double HashMap,
/// first over component TypeId, then over E.
pub struct ECMap<E: Hash + Eq = u32> {
    // Index to easier iterate over all entities
    entities: HashSet<E>,
    // The hashmap's value is a CList<C, E> where C has the same TypeId.
    components: HashMap<TypeId, Box<CompList<E>>>,
    last_entity: E,
}

impl<E: Hash + Eq + Default> ECMap<E> {
    /// Returns a new ECMap with a custom entity ID type. If you want to use
    /// ECMap::insert_entity (to generate unique entity IDs),
    /// use ECMap::new_u32, ECMap::new_usize or ECMap::new_u64 instead.
    pub fn new_custom() -> ECMap<E> { Default::default() }
}

impl<E: Hash + Eq + Any + Copy> ECMap<E> {

    fn clist<C: Any>(&self) -> Option<&CList<C, E>> {
        self.components.get(&TypeId::of::<C>()).map(|c|
            c.as_any().downcast_ref::<CList<C, E>>().unwrap())
    }

    fn clist_mut<C: Any>(&mut self) -> Option<&mut CList<C, E>> {
        self.components.get_mut(&TypeId::of::<C>()).map(|c|
            c.as_any_mut().downcast_mut::<CList<C, E>>().unwrap())
    }

    /// Inserts or replaces an entity component. The old component, if there
    /// was one, is returned.
    /// If the entity or component did not at all exist, it is also added.
    pub fn insert<C: Any>(&mut self, e: E, c: C) -> Option<C> {
        self.entities.insert(e);
        let q = self.components.entry(TypeId::of::<C>())
            .or_insert_with(|| Box::new(CList::<C, E>::new_nodbg()));
        q.as_any_mut().downcast_mut::<CList<C, E>>().unwrap().0.insert(e, c)
    }

    /// Inserts a component type and enables debugging for that component.
    /// In case the component type already exists, nothing is changed and false is returned.  
    pub fn insert_component<C: Any + Debug>(&mut self) -> bool {
        if self.contains_component::<C>() { return false }
        self.components.insert(TypeId::of::<C>(), Box::new(CList::<C, E>::new_dbg()));
        true
    }

    /// Returns a reference to an entity component, or None if it does not exist.
    pub fn get<C: Any>(&self, e: E) -> Option<&C> {
        self.clist::<C>().and_then(|s| s.0.get(&e))
    }

    /// Returns a mutable reference to an entity component, or None if it does not exist.
    pub fn get_mut<C: Any>(&mut self, e: E) -> Option<&mut C> {
        self.clist_mut::<C>().and_then(|s| s.0.get_mut(&e))
    }

    /// Check whether an entity component exists.
    pub fn contains<C: Any>(&self, e: E) -> bool {
        self.get::<C>(e).is_some()
    }

    /// Check whether an entity exists.
    pub fn contains_entity(&self, e: E) -> bool {
        self.entities.contains(&e)
    }

    /// Check whether a component exists.
    pub fn contains_component<C: Any>(&self) -> bool {
        self.components.contains_key(&TypeId::of::<C>())
    }

    /// Removes an entity component.
    pub fn remove<C: Any>(&mut self, e: E) -> bool {
        self.components.get_mut(&TypeId::of::<C>()).map(
            |s| s.remove(&e)).unwrap_or(false)
    }

    /// Remove an entity (existing components will no longer have that entity).
    pub fn remove_entity(&mut self, e: E) -> bool {
        if !self.entities.remove(&e) { return false };
        for (_, c) in self.components.iter_mut() { c.remove(&e); }
        true
    }

    /// Remove a component (existing entities will no longer have that component).
    pub fn remove_component<C: Any>(&mut self) -> bool {
        self.components.remove(&TypeId::of::<C>()).is_some()
    }

    /// Clones the component for all entities and returns a Vec of those.
    pub fn clone_with<C: Any + Clone>(&self) -> Vec<(E, C)> {
        self.iter_with::<C>().map(|(k, v)| (*k, v.clone())).collect() 
    }

    /// Iterates over all entities having a certain component.
    pub fn iter_with<C: Any>(&self) -> Iter<E, C> {
        let c = if let Some(s) = self.clist::<C>() { s } else { return Iter(None) };
        Iter(Some(c.0.iter()))
    }

    /// Iterates over all entities having a certain component, yielding mutable references.
    pub fn iter_mut_with<C: Any>(&mut self) -> IterMut<E, C> {
        let c = if let Some(s) = self.clist_mut::<C>() { s } else { return IterMut(None) };
        IterMut(Some(c.0.iter_mut()))
    }
}

impl<E: Hash + Eq + Debug> Debug for ECMap<E> {
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
    /// Returns a new ECMap with u32 as Entity ID.
    pub fn new() -> ECMap<u32> { Default::default() }

    /// Generates an Entity ID and inserts it.
    pub fn insert_entity(&mut self) -> u32 {
        self.last_entity += 1;
        self.entities.insert(self.last_entity);
        self.last_entity
    }
}

impl ECMap<usize> {
    /// Returns a new ECMap with usize as Entity ID.
    pub fn new_usize() -> ECMap<usize> { Default::default() }

    /// Generates an Entity ID and inserts it.
    pub fn insert_entity(&mut self) -> usize {
        self.last_entity += 1;
        self.entities.insert(self.last_entity);
        self.last_entity
    }
}

impl ECMap<u64> {
    /// Returns a new ECMap with u64 as Entity ID.
    pub fn new_u64() -> ECMap<u64> { Default::default() }

    /// Generates an Entity ID and inserts it.
    pub fn insert_entity(&mut self) -> u64 {
        self.last_entity += 1;
        self.entities.insert(self.last_entity);
        self.last_entity
    }
}

#[test]
fn do_test() {

    let mut e = ECMap::new_usize();
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

#[test]
fn str_as_id() {

    let mut e = ECMap::new_custom();

    #[derive(Debug, Eq, PartialEq, Hash)]
    struct Sound(&'static str);

    e.insert("Cat", Sound("Meow"));
    assert_eq!(e.get::<Sound>("Cat"), Some(&Sound("Meow")));
}
