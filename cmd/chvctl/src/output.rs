use clap::ValueEnum;
use serde_json::Value;
use tabled::{settings::Style, Table, Tabled};

#[derive(Debug, Clone, Default, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Yaml,
}

#[derive(Tabled)]
struct GenericRow {
    #[tabled(rename = "KEY")]
    key: String,
    #[tabled(rename = "VALUE")]
    value: String,
}

pub fn print_json(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

pub fn print_value(value: &Value, format: &OutputFormat) {
    match format {
        OutputFormat::Json => print_json(value),
        OutputFormat::Yaml => {
            // Simple YAML-like output (no extra dependency)
            print_yaml_like(value, 0);
        }
        OutputFormat::Table => {
            if let Some(obj) = value.as_object() {
                let rows: Vec<GenericRow> = obj
                    .iter()
                    .map(|(k, v)| GenericRow {
                        key: k.clone(),
                        value: format_value(v),
                    })
                    .collect();
                let table = Table::new(rows).with(Style::modern()).to_string();
                println!("{table}");
            } else {
                print_json(value);
            }
        }
    }
}

pub fn print_list(items: &[Value], columns: &[&str], format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(items).unwrap_or_default()
            );
        }
        OutputFormat::Yaml => {
            for item in items {
                print_yaml_like(item, 0);
                println!("---");
            }
        }
        OutputFormat::Table => {
            print_table(items, columns);
        }
    }
}

fn print_table(items: &[Value], columns: &[&str]) {
    if items.is_empty() {
        println!("No items found.");
        return;
    }

    // Build header
    let header: Vec<String> = columns.iter().map(|c| c.to_uppercase()).collect();

    // Build rows
    let mut rows: Vec<Vec<String>> = Vec::new();
    for item in items {
        let row: Vec<String> = columns
            .iter()
            .map(|col| {
                item.get(*col)
                    .map(format_value)
                    .unwrap_or_else(|| "-".to_string())
            })
            .collect();
        rows.push(row);
    }

    // Calculate column widths
    let mut widths: Vec<usize> = header.iter().map(|h| h.len()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if cell.len() > widths[i] {
                widths[i] = cell.len();
            }
        }
    }

    // Print header
    let header_str: Vec<String> = header
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:<width$}", h, width = widths[i]))
        .collect();
    println!("{}", header_str.join("  "));

    // Print separator
    let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    println!("{}", sep.join("  "));

    // Print rows
    for row in &rows {
        let row_str: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| format!("{:<width$}", cell, width = widths[i]))
            .collect();
        println!("{}", row_str.join("  "));
    }
}

fn format_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "-".to_string(),
        _ => v.to_string(),
    }
}

fn print_yaml_like(value: &Value, indent: usize) {
    let prefix = " ".repeat(indent);
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                match v {
                    Value::Object(_) | Value::Array(_) => {
                        println!("{prefix}{k}:");
                        print_yaml_like(v, indent + 2);
                    }
                    _ => {
                        println!("{prefix}{k}: {}", format_value(v));
                    }
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                println!("{prefix}-");
                print_yaml_like(item, indent + 2);
            }
        }
        _ => {
            println!("{prefix}{}", format_value(value));
        }
    }
}
