// cqam-core/src/register.rs

/// Core data types for classical and hybrid registers.

#[derive(Debug, Clone, PartialEq)]
pub enum CValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum HybridValue {
    Prob(f64),
    Dist(Vec<(CValue, f64)>),
}

#[derive(Debug, Clone)]
pub struct RegisterBank {
    pub c: std::collections::HashMap<String, CValue>,
    pub h: std::collections::HashMap<String, HybridValue>,
}

impl RegisterBank {
    pub fn new() -> Self {
        Self {
            c: std::collections::HashMap::new(),
            h: std::collections::HashMap::new(),
        }
    }

    pub fn load_c(&self, key: &str) -> Option<&CValue> {
        self.c.get(key)
    }

    pub fn store_c(&mut self, key: &str, val: CValue) {
        self.c.insert(key.to_string(), val);
    }

    pub fn load_h(&self, key: &str) -> Option<&HybridValue> {
        self.h.get(key)
    }

    pub fn store_h(&mut self, key: &str, val: HybridValue) {
        self.h.insert(key.to_string(), val);
    }
}