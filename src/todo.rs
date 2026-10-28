use std::fs;
use crate::state;

#[derive(Default, Clone)]
pub struct TodoItem {
    pub text: String,
    pub done: bool,
}

#[derive(Default, Clone)]
pub struct TodoList {
    pub items: Vec<TodoItem>,
}

impl TodoList {
    pub fn load() -> Self {
        let path = state::appdata_dir().join("todo.txt");
        let mut list = Self::default();
        if let Ok(txt) = fs::read_to_string(&path) {
            for line in txt.lines() {
                if line.is_empty() { continue; }
                let (done, text) = if let Some(rest) = line.strip_prefix("[x] ") {
                    (true, rest)
                } else if let Some(rest) = line.strip_prefix("[ ] ") {
                    (false, rest)
                } else {
                    (false, line)
                };
                list.items.push(TodoItem { text: text.to_string(), done });
            }
        }
        list
    }

    pub fn save(&self) {
        let dir = state::appdata_dir();
        let _ = fs::create_dir_all(&dir);
        let mut out = String::new();
        for it in &self.items {
            out.push_str(if it.done { "[x] " } else { "[ ] " });
            out.push_str(&it.text);
            out.push('\n');
        }
        let _ = fs::write(dir.join("todo.txt"), out);
    }

    pub fn add(&mut self, text: String) {
        self.items.push(TodoItem { text, done: false });
    }

    pub fn toggle(&mut self, idx: usize) {
        if let Some(it) = self.items.get_mut(idx) {
            it.done = !it.done;
        }
    }

    pub fn remove(&mut self, idx: usize) {
        if idx < self.items.len() {
            self.items.remove(idx);
        }
    }
}
