//! Helpers for the model catalog JSON referenced by `model_catalog_json`.
//!
//! The catalog is kept as a `serde_json::Value` (with the `preserve_order` feature)
//! so that fields this GUI does not understand yet are never dropped.

use serde_json::{Map, Value};

pub type JsonMap = Map<String, Value>;

/// `{"models": [ ... ]}`
pub fn models(doc: &Value) -> Option<&Vec<Value>> {
    doc.get("models")?.as_array()
}

pub fn models_mut(doc: &mut Value) -> Option<&mut Vec<Value>> {
    doc.get_mut("models")?.as_array_mut()
}

pub fn model_count(doc: &Value) -> usize {
    models(doc).map(Vec::len).unwrap_or(0)
}

pub fn slugs(doc: &Value) -> Vec<String> {
    models(doc)
        .map(|list| {
            list.iter()
                .filter_map(|m| m.get("slug").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub fn display_name(model: &Value) -> String {
    model
        .get("display_name")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| model.get("slug").and_then(Value::as_str))
        .unwrap_or("(未命名模型)")
        .to_string()
}

pub fn slug_of(model: &Value) -> String {
    model
        .get("slug")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub fn find_index(doc: &Value, slug: &str) -> Option<usize> {
    models(doc)?
        .iter()
        .position(|m| m.get("slug").and_then(Value::as_str) == Some(slug))
}

// ---------------------------------------------------------------------------
// Generic JSON path access (objects only)
// ---------------------------------------------------------------------------

pub fn get<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

pub fn set(value: &mut Value, path: &[&str], new: Value) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut current = value;
    for key in parents {
        if current.get(*key).is_none() {
            current[*key] = Value::Object(JsonMap::new());
        }
        let next = &mut current[*key];
        if !next.is_object() {
            *next = Value::Object(JsonMap::new());
        }
        current = next;
    }
    current[*last] = new;
}

pub fn remove(value: &mut Value, path: &[&str]) -> Option<Value> {
    let Some((last, parents)) = path.split_last() else {
        return None;
    };
    let mut current = value;
    for key in parents {
        current = current.get_mut(*key)?;
    }
    current.as_object_mut()?.remove(*last)
}

pub fn str_at(value: &Value, path: &[&str]) -> Option<String> {
    get(value, path).and_then(Value::as_str).map(str::to_string)
}

pub fn bool_at(value: &Value, path: &[&str]) -> Option<bool> {
    get(value, path).and_then(Value::as_bool)
}

pub fn i64_at(value: &Value, path: &[&str]) -> Option<i64> {
    get(value, path).and_then(Value::as_i64)
}

pub fn str_vec_at(value: &Value, path: &[&str]) -> Vec<String> {
    get(value, path)
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Effort levels supported by a model, e.g. `["low", "medium", "high"]`.
pub fn reasoning_levels(model: &Value) -> Vec<String> {
    get(model, &["supported_reasoning_levels"])
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|entry| entry.get("effort").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_reasoning_levels(model: &mut Value, levels: &[String]) {
    let descriptions: Vec<Value> = levels
        .iter()
        .map(|level| {
            let mut entry = JsonMap::new();
            entry.insert("effort".into(), Value::String(level.clone()));
            entry.insert(
                "description".into(),
                Value::String(reasoning_description(level)),
            );
            Value::Object(entry)
        })
        .collect();
    set(
        model,
        &["supported_reasoning_levels"],
        Value::Array(descriptions),
    );
}

pub fn reasoning_description(level: &str) -> String {
    match level {
        "none" => "不做额外推理，响应最快".into(),
        "minimal" => "极简推理，几乎不增加延迟".into(),
        "low" => "轻量推理，日常小任务足够快".into(),
        "medium" => "平衡速度与思考深度（推荐日常使用）".into(),
        "high" => "更深入的思考，适合复杂问题".into(),
        "xhigh" => "超高强度推理，适合难题".into(),
        "max" => "最大推理强度，最难的问题".into(),
        "ultra" => "最大推理强度，并自动把任务拆分给子代理".into(),
        other => format!("自定义等级 {other}"),
    }
    .to_string()
}

/// Every reasoning level Codex knows about, from fastest to deepest.
pub const ALL_REASONING_LEVELS: &[&str] = &[
    "none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra",
];

/// A minimal but valid model entry, used by "新建模型".
pub fn new_model_template(slug: &str, display_name: &str, provider: Option<&str>) -> Value {
    let mut model = JsonMap::new();
    model.insert("slug".into(), Value::String(slug.to_string()));
    model.insert(
        "display_name".into(),
        Value::String(display_name.to_string()),
    );
    model.insert(
        "description".into(),
        Value::String("通过 Codex 配置助手添加的自定义模型".into()),
    );
    if let Some(provider) = provider {
        model.insert("provider".into(), Value::String(provider.to_string()));
    } else {
        model.insert("provider".into(), Value::Null);
    }
    model.insert("visibility".into(), Value::String("list".into()));
    model.insert("supported_in_api".into(), Value::Bool(true));
    model.insert("priority".into(), Value::Number(50.into()));
    model.insert("shell_type".into(), Value::String("shell_command".into()));
    model.insert("context_window".into(), Value::Number(200_000.into()));
    model.insert("max_context_window".into(), Value::Number(200_000.into()));
    model.insert("input_modalities".into(), Value::Array(vec![
        Value::String("text".into()),
        Value::String("image".into()),
    ]));
    let mut truncation = JsonMap::new();
    truncation.insert("mode".into(), Value::String("tokens".into()));
    truncation.insert("limit".into(), Value::Number(25_600.into()));
    model.insert("truncation_policy".into(), Value::Object(truncation));
    model.insert("support_verbosity".into(), Value::Bool(false));
    model.insert("default_reasoning_summary".into(), Value::String("auto".into()));
    model.insert("experimental_supported_tools".into(), Value::Array(vec![]));
    model.insert(
        "default_reasoning_level".into(),
        Value::String("medium".into()),
    );
    let levels = ["low", "medium", "high"];
    model.insert(
        "supported_reasoning_levels".into(),
        Value::Array(
            levels
                .iter()
                .map(|level| {
                    let mut entry = JsonMap::new();
                    entry.insert("effort".into(), Value::String((*level).to_string()));
                    entry.insert(
                        "description".into(),
                        Value::String(reasoning_description(level)),
                    );
                    Value::Object(entry)
                })
                .collect(),
        ),
    );
    model.insert("base_instructions".into(), Value::String(String::new()));
    Value::Object(model)
}
