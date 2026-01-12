//! Mock 对象

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// 内存存储 Mock（用于测试 Repository）
#[derive(Debug, Default)]
pub struct InMemoryStore<T: Clone> {
    data: Arc<Mutex<HashMap<Uuid, T>>>,
}

impl<T: Clone> InMemoryStore<T> {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn insert(&self, id: Uuid, item: T) {
        let mut data = self.data.lock().unwrap();
        data.insert(id, item);
    }

    pub fn get(&self, id: &Uuid) -> Option<T> {
        let data = self.data.lock().unwrap();
        data.get(id).cloned()
    }

    pub fn remove(&self, id: &Uuid) -> Option<T> {
        let mut data = self.data.lock().unwrap();
        data.remove(id)
    }

    pub fn list(&self) -> Vec<T> {
        let data = self.data.lock().unwrap();
        data.values().cloned().collect()
    }

    pub fn count(&self) -> usize {
        let data = self.data.lock().unwrap();
        data.len()
    }

    pub fn clear(&self) {
        let mut data = self.data.lock().unwrap();
        data.clear();
    }
}

impl<T: Clone> Clone for InMemoryStore<T> {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
        }
    }
}
