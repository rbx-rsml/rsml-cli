#[derive(Debug, Clone)]
pub struct Arena<T> {
    pub data: Vec<T>,
    size: usize,
}

impl<'a, T> Arena<T> {
    pub fn new() -> Self {

        Self {
            data: vec![],
            size: 0,
        }
    }

    pub fn push(&mut self, value: T) -> usize {
        let this_idx = self.size;
        self.data.push(value);
        self.size += 1;
        this_idx
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.data.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.data.get_mut(index)
    }
}