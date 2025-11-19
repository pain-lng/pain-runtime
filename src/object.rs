// Object model for Pain runtime

use crate::allocator::Arena;
use std::collections::HashMap;
use std::ptr::NonNull;

/// Pain runtime value types
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    None,
    Object(ClassInstance), // Class instance
    List(Vec<Value>),      // Dynamic list
    Array(Vec<Value>),     // Fixed-size array (for now, same as list)
}

/// Class instance - stores field values
#[derive(Debug, Clone, PartialEq)]
pub struct ClassInstance {
    pub class_name: String,
    pub fields: HashMap<String, Value>,
}

impl ClassInstance {
    pub fn new(class_name: String) -> Self {
        Self {
            class_name,
            fields: HashMap::new(),
        }
    }

    pub fn get_field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }

    pub fn set_field(&mut self, name: String, value: Value) {
        self.fields.insert(name, value);
    }
}

/// Runtime object representation
pub struct Object {
    pub value: Value,
    // Future: metadata, type info, etc.
}

impl Object {
    pub fn new(value: Value) -> Self {
        Self { value }
    }

    pub fn as_int(&self) -> Option<i64> {
        match &self.value {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match &self.value {
            Value::Float(f) => Some(*f),
            Value::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match &self.value {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&String> {
        match &self.value {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&Vec<Value>> {
        match &self.value {
            Value::List(v) | Value::Array(v) => Some(v),
            _ => None,
        }
    }
}

/// Runtime context for managing objects and memory
pub struct Runtime {
    arena: Arena,
    gc: crate::gc::GarbageCollector,
}

impl Runtime {
    /// Create a new runtime instance
    pub fn new() -> Result<Self, &'static str> {
        Ok(Self {
            arena: Arena::new(1024 * 1024)?, // 1MB default
            gc: crate::gc::GarbageCollector::new(),
        })
    }

    /// Create a new runtime with custom arena size
    pub fn with_arena_size(size: usize) -> Result<Self, &'static str> {
        Ok(Self {
            arena: Arena::new(size)?,
            gc: crate::gc::GarbageCollector::new(),
        })
    }

    /// Create a new runtime with GC enabled and custom threshold
    pub fn with_gc_threshold(threshold: usize) -> Result<Self, &'static str> {
        Ok(Self {
            arena: Arena::new(1024 * 1024)?,
            gc: crate::gc::GarbageCollector::with_threshold(threshold),
        })
    }

    /// Allocate memory in the runtime arena
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        self.arena.allocate(size, align)
    }

    /// Reset the runtime arena (free all allocations)
    pub fn reset(&mut self) {
        self.arena.reset();
    }

    /// Get memory usage statistics
    pub fn memory_stats(&self) -> (usize, usize) {
        (self.arena.total_used(), self.arena.total_capacity())
    }

    /// Run garbage collection
    pub fn collect_garbage(&mut self) -> usize {
        self.gc.force_collect()
    }

    /// Get GC statistics
    pub fn gc_stats(&self) -> (usize, usize, usize) {
        self.gc.stats()
    }

    /// Manually trigger GC collection
    pub fn gc_collect(&mut self) {
        self.gc.collect();
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new().expect("Failed to create default runtime")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value() {
        use std::f64::consts::PI;

        let v1 = Value::Int(42);
        let v2 = Value::Float(PI);
        let _v3 = Value::Bool(true);
        let _v4 = Value::String("hello".to_string());

        assert_eq!(v1, Value::Int(42));
        assert_eq!(v2, Value::Float(PI));
    }

    #[test]
    fn test_class_instance() {
        let mut instance = ClassInstance::new("Point".to_string());
        instance.set_field("x".to_string(), Value::Int(10));
        instance.set_field("y".to_string(), Value::Int(20));

        assert_eq!(instance.get_field("x"), Some(&Value::Int(10)));
        assert_eq!(instance.get_field("y"), Some(&Value::Int(20)));
    }

    #[test]
    fn test_object() {
        let obj = Object::new(Value::Int(42));
        assert_eq!(obj.as_int(), Some(42));
        assert_eq!(obj.as_float(), Some(42.0));
    }

    #[test]
    fn test_runtime() {
        let mut rt = Runtime::new().unwrap();
        let ptr = rt.allocate(64, 8);
        assert!(ptr.is_some());

        let (used, capacity) = rt.memory_stats();
        assert!(used > 0);
        assert!(capacity > 0);
    }
}
