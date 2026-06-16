//! Convert a parsed `CHVArchitecture` into the UI canvas v1.0 graph payload.
//!
//! The canvas contract (`docs/specs/architecture-designer/contracts/graph-contract.md`)
//! expects `design_graph_json` to be a `{ version: "1.0", nodes, edges }` blob.
//! The seeder used to store the raw `CHVArchitecture` model JSON, which the
//! canvas could not render. This module builds a proper graph with MVP node
//! kinds, allowed edge types, and a deterministic grid layout.

use chv_architecture_validate::model::{
    CHVArchitecture, Datastore, Image, Instance, Network, Server, Template,
};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// v1.0 canvas graph payload.
#[derive(Clone, Debug, Serialize)]
struct GraphPayload {
    version: &'static str,
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

#[derive(Clone, Debug, Serialize)]
struct GraphNode {
    id: String,
    #[serde(rename = "type")]
    node_type: &'static str,
    position: Position,
    data: Value,
}

#[derive(Clone, Debug, Serialize)]
struct Position {
    x: f64,
    y: f64,
}

#[derive(Clone, Debug, Serialize)]
struct GraphEdge {
    id: String,
    #[serde(rename = "type")]
    edge_type: &'static str,
    source: String,
    target: String,
    data: EdgeData,
}

#[derive(Clone, Debug, Serialize)]
struct EdgeData {
    relationship: &'static str,
}

/// Node kind in the canvas vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum Kind {
    Host,
    Network,
    Datastore,
    Image,
    Template,
    Instance,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Host => "host",
            Kind::Network => "network",
            Kind::Datastore => "datastore",
            Kind::Image => "image",
            Kind::Template => "template",
            Kind::Instance => "instance",
        }
    }

    /// Horizontal column for the deterministic grid layout.
    fn column(self) -> f64 {
        match self {
            Kind::Host => 0.0,
            Kind::Network => 260.0,
            Kind::Datastore => 520.0,
            Kind::Image => 780.0,
            // Templates and instances sit in the centre column, lower down.
            Kind::Template => 390.0,
            Kind::Instance => 390.0,
        }
    }

    /// Vertical row offset for this kind.
    fn row_offset(self) -> f64 {
        match self {
            // Infrastructure nodes share the top band.
            Kind::Host | Kind::Network | Kind::Datastore | Kind::Image => 0.0,
            Kind::Template => 320.0,
            Kind::Instance => 540.0,
        }
    }
}

fn node_id(kind: Kind, name: &str) -> String {
    format!("node-{}-{}", kind.as_str(), name)
}

fn edge_id(source_id: &str, target_id: &str, edge_type: &str) -> String {
    format!("edge-{}-to-{}-{}", source_id, target_id, edge_type)
}

/// Build a v1.0 canvas graph JSON string from a `CHVArchitecture`.
///
/// Layout is deterministic so every fresh deployment renders the same starter
/// topology identically. Positions are coarse-grained; the operator rearranges
/// nodes on the canvas after cloning.
pub fn architecture_to_graph_json(arch: &CHVArchitecture) -> Result<String, serde_json::Error> {
    let graph = architecture_to_graph(arch);
    serde_json::to_string(&graph)
}

fn architecture_to_graph(arch: &CHVArchitecture) -> GraphPayload {
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    // Track per-kind counts so we can stack nodes vertically.
    let mut counts: HashMap<Kind, usize> = HashMap::new();

    let mut add_node = |kind: Kind, name: &str, data: Value| {
        let idx = *counts.entry(kind).and_modify(|c| *c += 1).or_insert(0);
        let y = kind.row_offset() + (idx as f64) * 110.0;
        nodes.push(GraphNode {
            id: node_id(kind, name),
            node_type: kind.as_str(),
            position: Position {
                x: kind.column(),
                y,
            },
            data,
        });
    };

    for server in &arch.servers {
        add_node(Kind::Host, &server.name, host_data(server));
    }
    for network in &arch.networks {
        add_node(Kind::Network, &network.name, network_data(network));
    }
    for datastore in &arch.datastores {
        add_node(Kind::Datastore, &datastore.name, datastore_data(datastore));
    }
    for image in &arch.images {
        add_node(Kind::Image, &image.name, image_data(image));
    }
    for template in &arch.templates {
        add_node(Kind::Template, &template.name, template_data(template));
    }
    for instance in &arch.instances {
        add_node(Kind::Instance, &instance.name, instance_data(instance));
    }

    // Only wire edges when the target node exists in the architecture. This
    // keeps the canvas graph valid even if a fixture contains a dangling ref.
    let node_ids: std::collections::HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();

    for instance in &arch.instances {
        let instance_id = node_id(Kind::Instance, &instance.name);

        if let Some(placement) = instance.placement.as_ref() {
            if let Some(server) = placement.server.as_deref() {
                let target = node_id(Kind::Host, server);
                if node_ids.contains(&target) {
                    edges.push(GraphEdge {
                        id: edge_id(&instance_id, &target, "placed_on"),
                        edge_type: "placed_on",
                        source: instance_id.clone(),
                        target,
                        data: EdgeData {
                            relationship: "placed_on",
                        },
                    });
                }
            }
        }

        if let Some(template_name) = instance.template.as_deref() {
            let target = node_id(Kind::Template, template_name);
            if node_ids.contains(&target) {
                edges.push(GraphEdge {
                    id: edge_id(&instance_id, &target, "uses_template"),
                    edge_type: "uses_template",
                    source: instance_id.clone(),
                    target,
                    data: EdgeData {
                        relationship: "uses_template",
                    },
                });
            }
        }

        for net in &instance.networks {
            let target = node_id(Kind::Network, &net.name);
            if node_ids.contains(&target) {
                edges.push(GraphEdge {
                    id: edge_id(&instance_id, &target, "attached_to_network"),
                    edge_type: "attached_to_network",
                    source: instance_id.clone(),
                    target,
                    data: EdgeData {
                        relationship: "attached_to_network",
                    },
                });
            }
        }
    }

    // Template -> image edges.
    for template in &arch.templates {
        let source = node_id(Kind::Template, &template.name);
        let target = node_id(Kind::Image, &template.image);
        if node_ids.contains(&target) {
            edges.push(GraphEdge {
                id: edge_id(&source, &target, "uses_image"),
                edge_type: "uses_image",
                source,
                target,
                data: EdgeData {
                    relationship: "uses_image",
                },
            });
        }
    }

    GraphPayload {
        version: "1.0",
        nodes,
        edges,
    }
}

fn host_data(server: &Server) -> Value {
    let mut map = Map::new();
    map.insert(
        "kind".to_string(),
        Value::String(Kind::Host.as_str().to_string()),
    );
    map.insert("name".to_string(), Value::String(server.name.clone()));
    if let Some(ip) = server.management_ip.as_deref() {
        map.insert("management_ip".to_string(), Value::String(ip.to_string()));
    }
    if let Some(role) = server.role.as_ref() {
        map.insert(
            "role".to_string(),
            Value::String(
                serde_json::to_string(role)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
            ),
        );
    }
    if let Some(res) = server.resources.as_ref() {
        if let Some(cpu) = res.cpu_cores {
            map.insert("cpu_cores".to_string(), Value::Number(cpu.into()));
        }
        if let Some(mem) = res.memory_gb {
            map.insert("memory_gb".to_string(), Value::Number(mem.into()));
        }
    }
    Value::Object(map)
}

fn network_data(network: &Network) -> Value {
    let mut map = Map::new();
    map.insert(
        "kind".to_string(),
        Value::String(Kind::Network.as_str().to_string()),
    );
    map.insert("name".to_string(), Value::String(network.name.clone()));
    map.insert(
        "type".to_string(),
        Value::String(
            serde_json::to_string(&network.network_type)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
        ),
    );
    if let Some(bridge) = network.bridge.as_deref() {
        map.insert("bridge".to_string(), Value::String(bridge.to_string()));
    }
    if let Some(vlan_id) = network.vlan_id {
        map.insert("vlan_id".to_string(), Value::Number(vlan_id.into()));
    }
    if let Some(cidr) = network.cidr.as_deref() {
        map.insert("cidr".to_string(), Value::String(cidr.to_string()));
    }
    if let Some(gateway) = network.gateway.as_deref() {
        map.insert("gateway".to_string(), Value::String(gateway.to_string()));
    }
    Value::Object(map)
}

fn datastore_data(datastore: &Datastore) -> Value {
    let mut map = Map::new();
    map.insert(
        "kind".to_string(),
        Value::String(Kind::Datastore.as_str().to_string()),
    );
    map.insert("name".to_string(), Value::String(datastore.name.clone()));
    map.insert(
        "type".to_string(),
        Value::String(
            serde_json::to_string(&datastore.datastore_type)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
        ),
    );
    if let Some(path) = datastore.path.as_deref() {
        map.insert("path".to_string(), Value::String(path.to_string()));
    }
    if let Some(pool) = datastore.pool.as_deref() {
        map.insert("pool".to_string(), Value::String(pool.to_string()));
    }
    Value::Object(map)
}

fn image_data(image: &Image) -> Value {
    let mut map = Map::new();
    map.insert(
        "kind".to_string(),
        Value::String(Kind::Image.as_str().to_string()),
    );
    map.insert("name".to_string(), Value::String(image.name.clone()));
    map.insert("source".to_string(), Value::String(image.source.clone()));
    map.insert(
        "format".to_string(),
        Value::String(
            serde_json::to_string(&image.format)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
        ),
    );
    if let Some(ds) = image.datastore.as_deref() {
        map.insert("datastore".to_string(), Value::String(ds.to_string()));
    }
    Value::Object(map)
}

fn template_data(template: &Template) -> Value {
    let mut map = Map::new();
    map.insert(
        "kind".to_string(),
        Value::String(Kind::Template.as_str().to_string()),
    );
    map.insert("name".to_string(), Value::String(template.name.clone()));
    map.insert("image".to_string(), Value::String(template.image.clone()));
    if let Some(cpu) = template.cpu {
        map.insert("cpu".to_string(), Value::Number(cpu.into()));
    }
    if let Some(mem) = template.memory_mb {
        map.insert("memory_mb".to_string(), Value::Number(mem.into()));
    }
    if let Some(disk) = template.disk_gb {
        map.insert("disk_gb".to_string(), Value::Number(disk.into()));
    }
    if let Some(ds) = template.datastore.as_deref() {
        map.insert("datastore".to_string(), Value::String(ds.to_string()));
    }
    if let Some(net) = template.network.as_deref() {
        map.insert("network".to_string(), Value::String(net.to_string()));
    }
    Value::Object(map)
}

fn instance_data(instance: &Instance) -> Value {
    let mut map = Map::new();
    map.insert(
        "kind".to_string(),
        Value::String(Kind::Instance.as_str().to_string()),
    );
    map.insert("name".to_string(), Value::String(instance.name.clone()));
    if let Some(template) = instance.template.as_deref() {
        map.insert("template".to_string(), Value::String(template.to_string()));
    }
    if let Some(placement) = instance.placement.as_ref() {
        if let Some(server) = placement.server.as_deref() {
            map.insert(
                "placement_server".to_string(),
                Value::String(server.to_string()),
            );
        }
    }
    if let Some(res) = instance.resources.as_ref() {
        if let Some(cpu) = res.cpu {
            map.insert("resources_cpu".to_string(), Value::Number(cpu.into()));
        }
        if let Some(mem) = res.memory_mb {
            map.insert("resources_memory_mb".to_string(), Value::Number(mem.into()));
        }
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::STARTER_FIXTURES;
    use chv_architecture_validate::parse_yaml;

    #[test]
    fn every_fixture_produces_valid_graph_payload() {
        for fixture in STARTER_FIXTURES {
            let model = parse_yaml(fixture.yaml).expect("fixture must parse");
            let json = architecture_to_graph_json(&model).expect("graph serializes");
            let parsed: serde_json::Value =
                serde_json::from_str(&json).expect("graph deserializes");
            assert_eq!(parsed["version"], "1.0");
            assert!(!parsed["nodes"].as_array().unwrap().is_empty());
            assert!(parsed["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|n| n["id"].is_string()));
            assert!(parsed["edges"].as_array().unwrap().iter().all(|e| {
                e["source"].is_string() && e["target"].is_string() && e["type"].is_string()
            }));
        }
    }

    #[test]
    fn single_vm_graph_has_expected_nodes_and_edges() {
        let fixture = STARTER_FIXTURES
            .iter()
            .find(|f| f.slug == "single-vm")
            .expect("single-vm fixture exists");
        let model = parse_yaml(fixture.yaml).expect("parse");
        let graph = architecture_to_graph(&model);

        let kinds: Vec<&str> = graph.nodes.iter().map(|n| n.node_type).collect();
        assert!(kinds.contains(&"host"));
        assert!(kinds.contains(&"network"));
        assert!(kinds.contains(&"image"));
        assert!(kinds.contains(&"template"));
        assert!(kinds.contains(&"instance"));

        let edge_types: Vec<&str> = graph.edges.iter().map(|e| e.edge_type).collect();
        assert!(edge_types.contains(&"placed_on"));
        assert!(edge_types.contains(&"attached_to_network"));
        assert!(edge_types.contains(&"uses_template"));
        assert!(edge_types.contains(&"uses_image"));
    }
}
