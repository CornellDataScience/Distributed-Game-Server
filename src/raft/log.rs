

use std::{fs::{self, File}, io::Write};
use serde_derive::{Serialize,Deserialize};


#[derive(Serialize, Deserialize, Debug)]
#[derive(Clone)]

pub struct LogEntry {
    term: u64,
    command: Command,
}

impl LogEntry {
    pub fn get_term(&self) -> u64 {
        self.term
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[derive(Clone)]
pub struct Command {
    key: String,
    value: i64,
}

#[derive(Debug)]
pub struct Log {
    update_cache_every: usize,
    path: String,
    pub cache: Vec<LogEntry>,
    commit_index: u32,
    last_applied: u32,
    next_index: u32,
    match_index: u32,
}

impl Log {
    
    pub fn new(path: String) -> Self {

        Self {
            update_cache_every: 5,
            path: path,
            cache: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            next_index: 0,
            match_index: 0,
        }
    }

    pub fn get_item(&self, index: usize) -> &LogEntry {
        &self.cache[index]
    }

    pub fn set_item (&mut self, index: usize, log: LogEntry) {
        self.cache[index] = log;
    }
    
    pub fn size(&self) -> usize {
        self.cache.len().clone()
    }

    pub fn write_entry(&mut self, entry: LogEntry) {
        let mut content = self.read();
        content.push(entry);

        self.write_all(content);

        if(self.size().rem_euclid(self.update_cache_every) == 0) {
            self.cache = self.read();
        }
    }
    
    pub fn write_all(&mut self, cache: Vec<LogEntry>) {
        let serialized = serde_json::to_string(&cache).unwrap();
        fs::write(&self.path, serialized)
            .expect("Unable to write file");
    }

    pub fn read(&self) -> Vec<LogEntry> {
        let contents = fs::read_to_string(&self.path)
            .expect("Should have been able to read the file");
        println!("Read: {}",contents);
        let deserialized = match serde_json::from_str(&contents)
        {
            Ok(contents) => contents,
            Err(e) => Vec::new(),
        };
        return deserialized;
    }

    pub fn erase_from(&mut self, index: usize) {
        let mut updated = Vec::new();

        for i in 0..index {
            updated.push(self.cache[i].clone());
        }

        self.write_all(updated);
        self.cache.clear();
    }

    pub fn last_log_index(&self) -> usize {
        self.size()-1
    }
    pub fn last_log_term(&self) -> u64 {
        if self.last_log_index() == 0 {
            return 0;
        } else {
            return self.cache[self.last_log_index()-1].term;
        }
    }
}