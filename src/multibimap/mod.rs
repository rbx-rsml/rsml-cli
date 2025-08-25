use std::{borrow::Borrow, collections::{btree_map::Entry, BTreeMap, HashSet}, fmt::Debug, hash::Hash, sync::Arc};

mod mem;
use mem::Wrapper;

pub use mem::Ref;

#[derive(Debug, Default)]
pub struct MultiBiMap<L, R> {
    pub left_to_right: BTreeMap<Ref<L>, HashSet<Ref<R>>>,
    pub right_to_left: BTreeMap<Ref<R>, HashSet<Ref<L>>>
}

impl<L, R> MultiBiMap<L, R>
where
    L: Eq + Hash + Debug + Ord, R: Eq + Hash + Debug + Ord,
{
    pub fn new() -> Self {
        Self {
            left_to_right: BTreeMap::new(),
            right_to_left: BTreeMap::new()
        }
    }

    pub fn insert(&mut self, left: L, right: R) -> (&mut HashSet<Ref<L>>, &mut HashSet<Ref<R>>) {
        let left = Ref(Arc::new(left));
        let right = Ref(Arc::new(right));

        let right_map = self.left_to_right.entry(left.clone()).or_insert(HashSet::new());
        right_map.insert(right.clone());

        let left_map = self.right_to_left.entry(right.clone()).or_insert(HashSet::new());
        left_map.insert(left.clone());

        (left_map, right_map)
    }

    pub fn insert_by_left(&mut self, left: L) -> &mut HashSet<Ref<R>> {
        let left = Ref(Arc::new(left));

        self.left_to_right.entry(left.clone()).or_insert(HashSet::new())
    }

    pub fn insert_by_right(&mut self, right: R) -> &mut HashSet<Ref<L>> {
        let right = Ref(Arc::new(right));

        self.right_to_left.entry(right.clone()).or_insert(HashSet::new())
    }

    pub fn remove_by_left(&mut self, left: L) {
        let left_ref = Ref(Arc::new(left));
        
        if let Some(right_set) = self.left_to_right.get(&left_ref) {
            for right_ref in right_set.iter() {
                if let Some(right_set) = self.right_to_left.get_mut(right_ref) {
                    if right_set.len() == 1 {
                        self.right_to_left.remove(right_ref);
                    } else {
                        right_set.remove(&left_ref);
                    }

                }
            }
        }
    
        self.left_to_right.remove(&left_ref);
    }

    pub fn remove_by_right(&mut self, right: R) {
        let right_ref = Ref(Arc::new(right));
        
        if let Some(left_set) = self.right_to_left.get(&right_ref) {
            for left_ref in left_set.iter() {
                if let Some(left_set) = self.left_to_right.get_mut(left_ref) {
                    if left_set.len() == 1 {
                        self.left_to_right.remove(left_ref);
                    } else {
                        left_set.remove(&right_ref);
                    }

                }
            }
        }
    
        self.right_to_left.remove(&right_ref);
    }

    pub fn get_by_left<Q>(&self, left: &Q) -> Option<&HashSet<Ref<R>>>
    where
        L: Borrow<Q>,
        Q: Eq + Hash + ?Sized + Ord,
    {
        self.left_to_right.get(Wrapper::wrap(left))
    }

    pub fn get_mut_by_left<Q>(&mut self, left: &Q) -> Option<&mut HashSet<Ref<R>>>
    where
        L: Borrow<Q>,
        Q: Eq + Hash + ?Sized + Ord,
    {
        self.left_to_right.get_mut(Wrapper::wrap(left))
    }

    pub fn entry_by_left(&mut self, left: L) -> Entry<'_, Ref<L>, HashSet<Ref<R>>> {
        let left = Ref(Arc::new(left));
    
        self.left_to_right.entry(left)
    }

    pub fn entry_by_right(&mut self, right: R) -> Entry<'_, Ref<R>, HashSet<Ref<L>>> {
        let right = Ref(Arc::new(right));
    
        self.right_to_left.entry(right)
    }

    pub fn get_by_right<Q>(&self, right: &Q) -> Option<&HashSet<Ref<L>>>
    where
        R: Borrow<Q>,
        Q: Eq + Hash + ?Sized + Ord,
    {
        self.right_to_left.get(Wrapper::wrap(right))
    }

    pub fn get_mut_by_right<Q>(&mut self, right: &Q) -> Option<&mut HashSet<Ref<L>>>
    where
        R: Borrow<Q>,
        Q: Eq + Hash + ?Sized + Ord,
    {
        self.right_to_left.get_mut(Wrapper::wrap(right))
    }
}


/*
    This MultiBiMap implementation is forked 
    from billyrieger's bimap implementation.

    Permission is hereby granted, free of charge, to any
    person obtaining a copy of this software and associated
    documentation files (the "Software"), to deal in the
    Software without restriction, including without
    limitation the rights to use, copy, modify, merge,
    publish, distribute, sublicense, and/or sell copies of
    the Software, and to permit persons to whom the Software
    is furnished to do so, subject to the following
    conditions:

    The above copyright notice and this permission notice
    shall be included in all copies or substantial portions
    of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
    ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
    TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
    PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
    SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
    CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
    OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
    IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.
*/