#![allow(dead_code)] // generic helpers kept as a small utility surface
//! Lossless helpers for reading and writing `config.toml` through `toml_edit`.
//!
//! Everything in here works on *paths* (`&["tui", "theme"]`) so that callers never
//! have to think about the difference between a table, a dotted table or a value.
//! Comments, key order and formatting of untouched keys are preserved.

use toml_edit::{Array, DocumentMut, InlineTable, Item, Table, Value};

/// Tables that are pure namespaces: they never hold scalars themselves, so they are
/// rendered implicitly (no bare `[model_providers]` header, only `[model_providers.x]`).
const IMPLICIT_NAMESPACES: &[&str] = &[
    "model_providers",
    "profiles",
    "mcp_servers",
    "plugins",
    "marketplaces",
    "projects",
];

/// Path based access to a parsed TOML document.
pub trait TomlPathExt {
    fn item_at(&self, path: &[&str]) -> Option<&Item>;
    fn item_at_mut(&mut self, path: &[&str]) -> Option<&mut Item>;
    fn value_at(&self, path: &[&str]) -> Option<&Value>;
    fn table_at(&self, path: &[&str]) -> Option<&Table>;
    fn table_at_mut(&mut self, path: &[&str]) -> Option<&mut Table>;
    /// Keys of the table at `path`, in document order.
    fn keys_at(&self, path: &[&str]) -> Vec<String>;
    fn set_value_at(&mut self, path: &[&str], value: Value);
    fn set_item_at(&mut self, path: &[&str], item: Item);
    fn remove_at(&mut self, path: &[&str]) -> bool;
    /// Make sure a table exists at `path`, creating (and headering) it if needed.
    fn ensure_table_at(&mut self, path: &[&str]) -> &mut Table;

    // Small typed conveniences -------------------------------------------------
    fn str_at(&self, path: &[&str]) -> Option<String>;
    fn bool_at(&self, path: &[&str]) -> Option<bool>;
    fn int_at(&self, path: &[&str]) -> Option<i64>;
    fn float_at(&self, path: &[&str]) -> Option<f64>;
    fn str_array_at(&self, path: &[&str]) -> Option<Vec<String>>;
    /// Raw TOML snippet of a value, for "what is actually in my file" views.
    fn raw_at(&self, path: &[&str]) -> Option<String>;
}

impl TomlPathExt for DocumentMut {
    fn item_at(&self, path: &[&str]) -> Option<&Item> {
        descend_table(&self.as_table(), path)
    }

    fn item_at_mut(&mut self, path: &[&str]) -> Option<&mut Item> {
        let (last, parents) = path.split_last()?;
        let table = descend_table_mut(self.as_table_mut(), parents)?;
        table.get_mut(*last)
    }

    fn value_at(&self, path: &[&str]) -> Option<&Value> {
        match self.item_at(path)? {
            Item::Value(value) => Some(value),
            _ => None,
        }
    }

    fn table_at(&self, path: &[&str]) -> Option<&Table> {
        match self.item_at(path)? {
            Item::Table(table) => Some(table),
            Item::Value(Value::InlineTable(_)) => None,
            _ => None,
        }
    }

    fn table_at_mut(&mut self, path: &[&str]) -> Option<&mut Table> {
        match self.item_at_mut(path)? {
            Item::Table(table) => Some(table),
            _ => None,
        }
    }

    fn keys_at(&self, path: &[&str]) -> Vec<String> {
        if path.is_empty() {
            return self.as_table().iter().map(|(k, _)| k.to_string()).collect();
        }
        match self.table_at(path) {
            Some(table) => table.iter().map(|(k, _)| k.to_string()).collect(),
            None => Vec::new(),
        }
    }

    fn set_value_at(&mut self, path: &[&str], value: Value) {
        self.set_item_at(path, Item::Value(value));
    }

    fn set_item_at(&mut self, path: &[&str], mut item: Item) {
        let Some((last, parents)) = path.split_last() else {
            return;
        };
        let table = ensure_table_chain(self.as_table_mut(), parents);
        if let Some(existing) = table.get_mut(last) {
            if let (Some(old), Some(new)) = (existing.as_value(), item.as_value_mut()) {
                *new.decor_mut() = old.decor().clone();
            }
            // Table::insert reformats the key, discarding its leading comments.
            // Replace only the item when the key already exists.
            *existing = item;
        } else {
            table.insert(last, item);
        }
    }

    fn remove_at(&mut self, path: &[&str]) -> bool {
        let Some((last, parents)) = path.split_last() else {
            return false;
        };
        match descend_table_mut(self.as_table_mut(), parents) {
            Some(table) => table.remove(*last).is_some(),
            None => false,
        }
    }

    fn ensure_table_at(&mut self, path: &[&str]) -> &mut Table {
        ensure_table_chain(self.as_table_mut(), path)
    }

    fn str_at(&self, path: &[&str]) -> Option<String> {
        self.value_at(path)?.as_str().map(str::to_string)
    }

    fn bool_at(&self, path: &[&str]) -> Option<bool> {
        self.value_at(path)?.as_bool()
    }

    fn int_at(&self, path: &[&str]) -> Option<i64> {
        self.value_at(path)?.as_integer()
    }

    fn float_at(&self, path: &[&str]) -> Option<f64> {
        self.value_at(path)?.as_float()
    }

    fn str_array_at(&self, path: &[&str]) -> Option<Vec<String>> {
        let array = self.value_at(path)?.as_array()?;
        Some(
            array
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
        )
    }

    fn raw_at(&self, path: &[&str]) -> Option<String> {
        Some(match self.item_at(path)? {
            Item::Value(value) => value.to_string(),
            Item::Table(table) => table.to_string(),
            Item::ArrayOfTables(tables) => tables.to_string(),
            Item::None => return None,
        })
    }
}

fn descend_table<'a>(mut table: &'a Table, path: &[&str]) -> Option<&'a Item> {
    let (last, parents) = path.split_last()?;
    for key in parents {
        table = match table.get(*key)? {
            Item::Table(inner) => inner,
            _ => return None,
        };
    }
    table.get(*last)
}

fn descend_table_mut<'a>(mut table: &'a mut Table, path: &[&str]) -> Option<&'a mut Table> {
    for key in path {
        table = match table.get_mut(*key)? {
            Item::Table(inner) => inner,
            _ => return None,
        };
    }
    Some(table)
}

fn ensure_table_chain<'a>(mut table: &'a mut Table, path: &[&str]) -> &'a mut Table {
    for key in path {
        let entry = table.entry(key).or_insert(Item::Table(Table::new()));
        if !matches!(entry, Item::Table(_)) {
            // A scalar was in the way: replace it with a table.
            *entry = Item::Table(Table::new());
        }
        let Item::Table(inner) = entry else {
            unreachable!()
        };
        if IMPLICIT_NAMESPACES.contains(key) && inner.is_empty() {
            inner.set_implicit(true);
        }
        table = inner;
    }
    table
}

// ---------------------------------------------------------------------------
// Value construction helpers
// ---------------------------------------------------------------------------

pub fn value_str(s: &str) -> Value {
    Value::from(s.to_string())
}

pub fn value_opt_str(s: &str) -> Option<Value> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(Value::from(trimmed.to_string()))
    }
}

pub fn value_int(i: i64) -> Value {
    Value::from(i)
}

pub fn value_bool(b: bool) -> Value {
    Value::from(b)
}

pub fn value_float(f: f64) -> Value {
    Value::from(f)
}

pub fn value_str_array(items: &[String]) -> Value {
    let mut array = Array::new();
    for item in items {
        array.push(item.as_str());
    }
    Value::Array(array)
}

pub fn inline_table(pairs: &[(&str, String)]) -> Value {
    let mut table = InlineTable::new();
    for (key, value) in pairs {
        table.insert(*key, Value::from(value.clone()).into());
    }
    Value::InlineTable(table)
}

/// Reads a string map that may be stored either inline (`{ a = "b" }`) or as a table.
pub fn read_string_map(item: Option<&Item>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    match item {
        Some(Item::Value(Value::InlineTable(table))) => {
            for (key, value) in table.iter() {
                out.push((key.to_string(), value_to_string(value)));
            }
        }
        Some(Item::Table(table)) => {
            for (key, value) in table.iter() {
                if let Item::Value(value) = value {
                    out.push((key.to_string(), value_to_string(value)));
                }
            }
        }
        _ => {}
    }
    out
}

fn value_to_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> DocumentMut {
        "# top comment\nmodel = \"a\"\n\n[model_providers.x]\nbase_url = \"http://x/v1\"\n"
            .parse::<DocumentMut>()
            .unwrap()
    }

    #[test]
    fn reads_values_by_path() {
        let d = doc();
        assert_eq!(d.str_at(&["model"]).as_deref(), Some("a"));
        assert_eq!(
            d.str_at(&["model_providers", "x", "base_url"]).as_deref(),
            Some("http://x/v1")
        );
        assert_eq!(d.keys_at(&["model_providers"]), vec!["x".to_string()]);
    }

    #[test]
    fn set_creates_nested_tables_and_keeps_comments() {
        let mut d = doc();
        d.set_value_at(&["model_providers", "y", "wire_api"], value_str("chat"));
        d.set_value_at(&["features", "hooks"], value_bool(true));
        let text = d.to_string();
        assert!(
            text.contains("# top comment"),
            "comments must survive: {text}"
        );
        assert!(text.contains("[model_providers.y]"), "{text}");
        assert!(text.contains("wire_api = \"chat\""), "{text}");
        assert!(text.contains("[features]"), "{text}");
        assert!(text.contains("hooks = true"), "{text}");
        // namespaces that only hold tables stay implicit
        assert!(!text.contains("\n[model_providers]\n"), "{text}");
    }

    #[test]
    fn remove_drops_keys() {
        let mut d = doc();
        assert!(d.remove_at(&["model_providers", "x", "base_url"]));
        assert!(d.str_at(&["model_providers", "x", "base_url"]).is_none());
        assert!(!d.remove_at(&["nope", "nothing"]));
    }

    #[test]
    fn arrays_and_maps_round_trip() {
        let mut d = doc();
        d.set_value_at(
            &["tui", "status_line"],
            value_str_array(&["model".to_string(), "git-branch".to_string()]),
        );
        assert_eq!(
            d.str_array_at(&["tui", "status_line"]),
            Some(vec!["model".to_string(), "git-branch".to_string()])
        );
        let text = d.to_string();
        assert!(
            text.contains("status_line = [\"model\", \"git-branch\"]"),
            "{text}"
        );
    }
}
