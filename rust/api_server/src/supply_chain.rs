//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Supply Chain Risk Propagation & Graph Intelligence
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{Duration as ChronoDuration, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

use crate::models::supply_chain::SupplyChainRiskItem;

/// Global default SupplyChainGraph singleton.
pub static GLOBAL_SUPPLY_CHAIN_GRAPH: Lazy<Arc<SupplyChainGraph>> =
    Lazy::new(|| Arc::new(SupplyChainGraph::from_env()));

/// A single directed edge representing a supply chain relationship in the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyChainEdge {
    /// Related company ticker symbol.
    pub target: String,
    /// Type of relationship relative to the source company ("supplier", "customer", "partner", "competitor").
    pub relationship_type: String,
    /// Quantitative strength or dependency weight of the relationship [0.0, 1.0].
    pub strength: f64,
    /// Whether the relationship is currently active (false if divested / terminated).
    pub is_active: bool,
}

/// Intermediate result from graph traversal.
#[derive(Debug, Clone)]
pub struct TraversedNode {
    pub ticker: String,
    pub relationship_type: String,
    pub depth: u32,
    pub path_strength: f64,
}

/// A corporate event record associated with a company in the supply chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorporateNegativeEvent {
    pub ticker: String,
    pub event_type: String,
    pub event_description: String,
    pub days_ago: u32,
    pub severity_weight: Option<f64>,
}

/// In-memory graph adjacency index and corporate event risk scoring engine.
#[derive(Debug, Clone)]
pub struct SupplyChainGraph {
    /// Ticker -> List of outgoing relationship edges
    pub adjacency_list: HashMap<String, Vec<SupplyChainEdge>>,
    /// Event category -> Base risk severity weight
    pub event_weights: HashMap<String, f64>,
    /// Pre-loaded / active corporate events across supply chain entities
    pub corporate_events: Vec<CorporateNegativeEvent>,
}

impl Default for SupplyChainGraph {
    fn default() -> Self {
        Self::from_env()
    }
}

impl SupplyChainGraph {
    /// Create an empty graph.
    pub fn empty() -> Self {
        Self {
            adjacency_list: HashMap::new(),
            event_weights: Self::default_event_weights(),
            corporate_events: Vec::new(),
        }
    }

    /// Default institutional event severity weights.
    pub fn default_event_weights() -> HashMap<String, f64> {
        let mut weights = HashMap::new();
        weights.insert("Bankruptcy".to_string(), 1.00);
        weights.insert("Delisting".to_string(), 0.95);
        weights.insert("Regulatory Investigation".to_string(), 0.90);
        weights.insert("Financial Restatement".to_string(), 0.85);
        weights.insert("Supply Chain Disruption".to_string(), 0.85);
        weights.insert("Earnings Warning".to_string(), 0.80);
        weights.insert("FDA Rejection".to_string(), 0.80);
        weights.insert("CEO Change".to_string(), 0.70);
        weights.insert("Restructuring".to_string(), 0.65);
        weights.insert("M&A".to_string(), 0.50);
        weights.insert("Default".to_string(), 0.60);
        weights
    }

    /// Load supply chain graph from configuration files or fallback defaults.
    pub fn from_env() -> Self {
        let map_paths = if let Ok(custom_path) = env::var("SUPPLY_CHAIN_MAP_PATH") {
            vec![PathBuf::from(custom_path)]
        } else {
            vec![
                PathBuf::from("./config/supply_chain_map.json"),
                PathBuf::from("../config/supply_chain_map.json"),
                PathBuf::from("../../config/supply_chain_map.json"),
                PathBuf::from("d:/FinText-Alpha-Vectorizer/config/supply_chain_map.json"),
                PathBuf::from("D:\\FinText-Alpha-Vectorizer\\config\\supply_chain_map.json"),
            ]
        };

        let events_paths = if let Ok(custom_path) = env::var("SUPPLY_CHAIN_EVENTS_PATH") {
            vec![PathBuf::from(custom_path)]
        } else {
            vec![
                PathBuf::from("./config/supply_chain_events.json"),
                PathBuf::from("../config/supply_chain_events.json"),
                PathBuf::from("../../config/supply_chain_events.json"),
                PathBuf::from("d:/FinText-Alpha-Vectorizer/config/supply_chain_events.json"),
                PathBuf::from("D:\\FinText-Alpha-Vectorizer\\config\\supply_chain_events.json"),
            ]
        };

        let map_path = map_paths.into_iter().find(|p| p.is_file());
        let events_path = events_paths.into_iter().find(|p| p.is_file());

        let mut graph = Self::empty();

        if let Some(ref mp) = map_path {
            info!(
                "[SupplyChainGraph] Loading supply chain map from: {}",
                mp.display()
            );
            graph.load_map_file(mp);
        } else {
            warn!("[SupplyChainGraph] No supply_chain_map.json found. Initializing built-in core graph.");
            graph.load_builtin_graph();
        }

        if let Some(ref ep) = events_path {
            info!(
                "[SupplyChainGraph] Loading supply chain events from: {}",
                ep.display()
            );
            graph.load_events_file(ep);
        } else {
            info!("[SupplyChainGraph] Initializing default institutional corporate risk events.");
            graph.load_builtin_events();
        }

        info!(
            "[SupplyChainGraph] Initialized with {} companies in graph and {} corporate risk events",
            graph.adjacency_list.len(),
            graph.corporate_events.len()
        );

        graph
    }

    /// Load inter-company relationships from `config/supply_chain_map.json`.
    pub fn load_map_file(&mut self, path: &Path) {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                warn!("[SupplyChainGraph] Failed to read supply chain map: {}", e);
                self.load_builtin_graph();
                return;
            }
        };

        #[derive(Deserialize)]
        struct MapEntry {
            related: Vec<String>,
            relation: String,
        }

        let map: HashMap<String, MapEntry> = match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(e) => {
                warn!(
                    "[SupplyChainGraph] JSON parse error in supply chain map: {}",
                    e
                );
                self.load_builtin_graph();
                return;
            }
        };

        for (ticker_raw, entry) in map {
            let ticker = ticker_raw.trim().to_uppercase();
            let relation = entry.relation.trim().to_lowercase();

            for rel_raw in entry.related {
                let rel_ticker = rel_raw.trim().to_uppercase();
                if rel_ticker.is_empty() || rel_ticker == ticker {
                    continue;
                }

                // If ticker is a "supplier" to rel_ticker:
                // Ticker's perspective -> rel_ticker is a "customer"
                // rel_ticker's perspective -> ticker is a "supplier"
                if relation == "supplier" {
                    self.add_edge(&ticker, &rel_ticker, "customer", 0.85, true);
                    self.add_edge(&rel_ticker, &ticker, "supplier", 0.85, true);
                } else if relation == "customer" {
                    // Ticker is a "customer" of rel_ticker:
                    // Ticker's perspective -> rel_ticker is a "supplier"
                    // rel_ticker's perspective -> ticker is a "customer"
                    self.add_edge(&ticker, &rel_ticker, "supplier", 0.85, true);
                    self.add_edge(&rel_ticker, &ticker, "customer", 0.85, true);
                } else if relation == "partner" {
                    self.add_edge(&ticker, &rel_ticker, "partner", 0.80, true);
                    self.add_edge(&rel_ticker, &ticker, "partner", 0.80, true);
                } else if relation == "competitor" {
                    self.add_edge(&ticker, &rel_ticker, "competitor", 0.70, true);
                    self.add_edge(&rel_ticker, &ticker, "competitor", 0.70, true);
                }
            }
        }
    }

    /// Load relationship events and explicit weights from `config/supply_chain_events.json`.
    pub fn load_events_file(&mut self, path: &Path) {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                warn!(
                    "[SupplyChainGraph] Failed to read supply chain events: {}",
                    e
                );
                self.load_builtin_events();
                return;
            }
        };

        #[derive(Deserialize)]
        struct EventEntry {
            #[serde(default)]
            source_company: Option<String>,
            #[serde(default)]
            target_company: Option<String>,
            #[serde(default)]
            relationship_type: Option<String>,
            #[serde(default)]
            strength: Option<f64>,
            #[serde(default)]
            is_closure: Option<bool>,
        }

        let events: Vec<EventEntry> = match serde_json::from_str(&content) {
            Ok(evs) => evs,
            Err(e) => {
                warn!(
                    "[SupplyChainGraph] JSON parse error in supply chain events: {}",
                    e
                );
                self.load_builtin_events();
                return;
            }
        };

        for ev in events {
            if let (Some(src), Some(tgt)) = (ev.source_company, ev.target_company) {
                let src_t = src.trim().to_uppercase();
                let tgt_t = tgt.trim().to_uppercase();
                let rel_type = ev
                    .relationship_type
                    .unwrap_or_else(|| "supplier".to_string())
                    .to_lowercase();
                let strength = ev.strength.unwrap_or(0.85);
                let is_active = !ev.is_closure.unwrap_or(false) && strength > 0.0;

                if rel_type == "supplier" {
                    // src is supplier to tgt:
                    // tgt's perspective -> src is "supplier"
                    // src's perspective -> tgt is "customer"
                    self.add_edge(&tgt_t, &src_t, "supplier", strength, is_active);
                    self.add_edge(&src_t, &tgt_t, "customer", strength, is_active);
                } else if rel_type == "partner" {
                    self.add_edge(&tgt_t, &src_t, "partner", strength, is_active);
                    self.add_edge(&src_t, &tgt_t, "partner", strength, is_active);
                } else if rel_type == "customer" {
                    self.add_edge(&tgt_t, &src_t, "customer", strength, is_active);
                    self.add_edge(&src_t, &tgt_t, "supplier", strength, is_active);
                }
            }
        }

        self.load_builtin_events();
    }

    /// Add or update a directed edge in the adjacency list.
    pub fn add_edge(
        &mut self,
        from: &str,
        to: &str,
        rel_type: &str,
        strength: f64,
        is_active: bool,
    ) {
        let edges = self.adjacency_list.entry(from.to_string()).or_default();
        if let Some(existing) = edges.iter_mut().find(|e| e.target == to) {
            existing.relationship_type = rel_type.to_string();
            existing.strength = strength;
            existing.is_active = is_active;
        } else {
            edges.push(SupplyChainEdge {
                target: to.to_string(),
                relationship_type: rel_type.to_string(),
                strength,
                is_active,
            });
        }
    }

    /// Built-in fallback graph with standard institutional supply chain connections.
    pub fn load_builtin_graph(&mut self) {
        // TSMC / TSM (Core Semiconductor Foundry)
        self.add_edge("AAPL", "TSM", "supplier", 0.95, true);
        self.add_edge("TSM", "AAPL", "customer", 0.95, true);
        self.add_edge("NVDA", "TSM", "supplier", 0.90, true);
        self.add_edge("TSM", "NVDA", "customer", 0.90, true);
        self.add_edge("AMD", "TSM", "supplier", 0.88, true);
        self.add_edge("TSM", "AMD", "customer", 0.88, true);

        // ASML (Tier-2 Supplier to TSM and INTC)
        self.add_edge("TSM", "ASML", "supplier", 0.92, true);
        self.add_edge("ASML", "TSM", "customer", 0.92, true);
        self.add_edge("INTC", "ASML", "supplier", 0.88, true);
        self.add_edge("ASML", "INTC", "customer", 0.88, true);

        // Apple Component Suppliers
        self.add_edge("AAPL", "QCOM", "supplier", 0.85, true);
        self.add_edge("QCOM", "AAPL", "customer", 0.85, true);
        self.add_edge("AAPL", "AVGO", "supplier", 0.85, true);
        self.add_edge("AVGO", "AAPL", "customer", 0.85, true);

        // AI Alliance
        self.add_edge("NVDA", "MSFT", "partner", 0.85, true);
        self.add_edge("MSFT", "NVDA", "partner", 0.85, true);

        // Automotive EV Supply Chain
        self.add_edge("TSLA", "RIVN", "competitor", 0.70, true);
        self.add_edge("TSLA", "LCID", "competitor", 0.70, true);
        self.add_edge("TSLA", "CATL", "supplier", 0.90, true);
        self.add_edge("CATL", "TSLA", "customer", 0.90, true);

        // Aerospace
        self.add_edge("BA", "SPR", "supplier", 0.92, true);
        self.add_edge("SPR", "BA", "customer", 0.92, true);
    }

    /// Load default realistic corporate negative events.
    pub fn load_builtin_events(&mut self) {
        self.corporate_events = vec![
            CorporateNegativeEvent {
                ticker: "TSM".to_string(),
                event_type: "Supply Chain Disruption".to_string(),
                event_description: "Advanced fab tooling recalibration and wafer supply bottleneck temporarily impact 3nm allocation.".to_string(),
                days_ago: 8,
                severity_weight: Some(0.85),
            },
            CorporateNegativeEvent {
                ticker: "ASML".to_string(),
                event_type: "Regulatory Investigation".to_string(),
                event_description: "Regulatory export licensing review expands scrutiny over advanced semiconductor lithography equipment shipments.".to_string(),
                days_ago: 14,
                severity_weight: Some(0.90),
            },
            CorporateNegativeEvent {
                ticker: "INTC".to_string(),
                event_type: "Earnings Warning".to_string(),
                event_description: "Revenue and gross margin warning: Foundry segment reports delayed fab tool delivery and customer migration timeline adjustments.".to_string(),
                days_ago: 15,
                severity_weight: Some(0.80),
            },
            CorporateNegativeEvent {
                ticker: "NVDA".to_string(),
                event_type: "CEO Change".to_string(),
                event_description: "Executive leadership transition: Chief Operating Officer announces retirement; board appoints Co-President as successor.".to_string(),
                days_ago: 3,
                severity_weight: Some(0.70),
            },
            CorporateNegativeEvent {
                ticker: "QCOM".to_string(),
                event_type: "Regulatory Investigation".to_string(),
                event_description: "FTC regulatory review launched into mobile modem patent licensing practices and exclusivity agreements.".to_string(),
                days_ago: 12,
                severity_weight: Some(0.90),
            },
            CorporateNegativeEvent {
                ticker: "AVGO".to_string(),
                event_type: "Regulatory Investigation".to_string(),
                event_description: "Antitrust inquiry opened into enterprise infrastructure software bundle renewal agreements.".to_string(),
                days_ago: 18,
                severity_weight: Some(0.90),
            },
            CorporateNegativeEvent {
                ticker: "MSFT".to_string(),
                event_type: "Earnings Warning".to_string(),
                event_description: "Preliminary revenue guidance adjustment for Intelligent Cloud segment reflecting accelerated datacenter capacity investments.".to_string(),
                days_ago: 4,
                severity_weight: Some(0.80),
            },
            CorporateNegativeEvent {
                ticker: "AMZN".to_string(),
                event_type: "Regulatory Investigation".to_string(),
                event_description: "Federal Trade Commission and Department of Justice regulatory review update regarding cloud AI infrastructure multi-tenant pricing.".to_string(),
                days_ago: 5,
                severity_weight: Some(0.90),
            },
            CorporateNegativeEvent {
                ticker: "META".to_string(),
                event_type: "Restructuring".to_string(),
                event_description: "Operational efficiency restructuring plan involving datacenter architecture consolidation with estimated pre-tax charges of $650 million.".to_string(),
                days_ago: 6,
                severity_weight: Some(0.65),
            },
            CorporateNegativeEvent {
                ticker: "TSLA".to_string(),
                event_type: "CEO Change".to_string(),
                event_description: "Board of Directors appoints new Chief Accounting Officer following completion of global automated manufacturing expansion audit.".to_string(),
                days_ago: 7,
                severity_weight: Some(0.70),
            },
            CorporateNegativeEvent {
                ticker: "BBBYQ".to_string(),
                event_type: "Bankruptcy".to_string(),
                event_description: "Voluntary petition for Chapter 11 bankruptcy protection filed in the United States Bankruptcy Court for the District of New Jersey.".to_string(),
                days_ago: 20,
                severity_weight: Some(1.00),
            },
            CorporateNegativeEvent {
                ticker: "SHOP".to_string(),
                event_type: "Restructuring".to_string(),
                event_description: "Global workforce reduction and operational streamlining across merchant delivery network.".to_string(),
                days_ago: 10,
                severity_weight: Some(0.65),
            },
            CorporateNegativeEvent {
                ticker: "WMT".to_string(),
                event_type: "Earnings Warning".to_string(),
                event_description: "Earnings guidance lowered due to discretionary consumer spending slowdown and supply chain freight costs.".to_string(),
                days_ago: 11,
                severity_weight: Some(0.80),
            },
            CorporateNegativeEvent {
                ticker: "RIVN".to_string(),
                event_type: "Earnings Warning".to_string(),
                event_description: "Annual vehicle delivery target reduced due to battery supplier component shortages and line upgrades.".to_string(),
                days_ago: 16,
                severity_weight: Some(0.80),
            },
            CorporateNegativeEvent {
                ticker: "LCID".to_string(),
                event_type: "CEO Change".to_string(),
                event_description: "Chief Executive Officer steps down; interim leadership committee appointed pending executive search.".to_string(),
                days_ago: 22,
                severity_weight: Some(0.70),
            },
            CorporateNegativeEvent {
                ticker: "BA".to_string(),
                event_type: "Regulatory Investigation".to_string(),
                event_description: "FAA regulatory investigation into fuselage assembly quality controls and supplier safety compliance.".to_string(),
                days_ago: 9,
                severity_weight: Some(0.90),
            },
            CorporateNegativeEvent {
                ticker: "SPR".to_string(),
                event_type: "Financial Restatement".to_string(),
                event_description: "Audit committee identifies accounting revisions in aerostructures manufacturing contract revenues.".to_string(),
                days_ago: 13,
                severity_weight: Some(0.85),
            },
        ];
    }

    /// Breadth-First-Search (BFS) traversal from the focal ticker up to `depth`.
    pub fn traverse_graph(&self, focal_ticker: &str, max_depth: u32) -> Vec<TraversedNode> {
        let root = focal_ticker.trim().to_uppercase();
        let mut results = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(root.clone());

        // Queue tuple: (current_ticker, current_depth, accumulated_strength, root_relation_type)
        let mut queue: VecDeque<(String, u32, f64, String)> = VecDeque::new();

        // Initialize queue with direct depth-1 neighbors
        if let Some(edges) = self.adjacency_list.get(&root) {
            for edge in edges {
                if edge.is_active && !visited.contains(&edge.target) {
                    visited.insert(edge.target.clone());
                    let rel_type = edge.relationship_type.clone();
                    let strength = edge.strength.clamp(0.01, 1.0);

                    results.push(TraversedNode {
                        ticker: edge.target.clone(),
                        relationship_type: rel_type.clone(),
                        depth: 1,
                        path_strength: strength,
                    });

                    if max_depth > 1 {
                        queue.push_back((edge.target.clone(), 1, strength, rel_type));
                    }
                }
            }
        }

        // Traverse deeper tiers (depth 2 and 3)
        while let Some((curr_ticker, curr_depth, acc_strength, primary_rel)) = queue.pop_front() {
            if curr_depth >= max_depth {
                continue;
            }

            if let Some(edges) = self.adjacency_list.get(&curr_ticker) {
                for edge in edges {
                    if edge.is_active && !visited.contains(&edge.target) {
                        visited.insert(edge.target.clone());
                        let next_depth = curr_depth + 1;
                        let next_strength =
                            (acc_strength * edge.strength.clamp(0.01, 1.0)).clamp(0.01, 1.0);

                        results.push(TraversedNode {
                            ticker: edge.target.clone(),
                            relationship_type: primary_rel.clone(),
                            depth: next_depth,
                            path_strength: next_strength,
                        });

                        if next_depth < max_depth {
                            queue.push_back((
                                edge.target.clone(),
                                next_depth,
                                next_strength,
                                primary_rel.clone(),
                            ));
                        }
                    }
                }
            }
        }

        results
    }

    /// Retrieve negative events for a ticker within the lookback window `event_days`.
    pub fn get_events_for_ticker(
        &self,
        ticker: &str,
        event_days: u32,
    ) -> Vec<&CorporateNegativeEvent> {
        let norm = ticker.trim().to_uppercase();
        self.corporate_events
            .iter()
            .filter(|e| e.ticker == norm && e.days_ago <= event_days)
            .collect()
    }

    /// Compute supply chain risk alerts for a focal company.
    pub fn compute_risk_alerts(
        &self,
        focal_ticker: &str,
        depth: u32,
        event_days: u32,
        min_risk_score: f64,
        limit: usize,
    ) -> Vec<SupplyChainRiskItem> {
        let root = focal_ticker.trim().to_uppercase();
        let traversed_nodes = self.traverse_graph(&root, depth);
        let today = Utc::now().date_naive();

        let mut alerts = Vec::new();

        for node in traversed_nodes {
            let events = self.get_events_for_ticker(&node.ticker, event_days);

            for event in events {
                // Determine event severity weight
                let event_weight = event.severity_weight.unwrap_or_else(|| {
                    self.event_weights
                        .get(&event.event_type)
                        .copied()
                        .unwrap_or(0.60)
                });

                // Risk Score Formula:
                // risk_score = event_weight * relationship_strength * (1.0 / depth)
                let depth_decay = 1.0 / (node.depth as f64);
                let raw_score = event_weight * node.path_strength * depth_decay;
                let risk_score = (raw_score.clamp(0.0, 1.0) * 10000.0).round() / 10000.0;

                if risk_score >= min_risk_score {
                    let event_date = (today - ChronoDuration::days(event.days_ago as i64))
                        .format("%Y-%m-%d")
                        .to_string();

                    alerts.push(SupplyChainRiskItem {
                        focal_ticker: root.clone(),
                        related_ticker: node.ticker.clone(),
                        relationship_type: node.relationship_type.clone(),
                        depth: node.depth,
                        event_type: event.event_type.clone(),
                        event_description: event.event_description.clone(),
                        event_date,
                        risk_score,
                    });
                }
            }
        }

        // Rank alerts:
        // 1. risk_score descending
        // 2. depth ascending (closer tiers first)
        // 3. event_date descending
        alerts.sort_by(|a, b| {
            b.risk_score
                .partial_cmp(&a.risk_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.depth.cmp(&b.depth))
                .then_with(|| b.event_date.cmp(&a.event_date))
        });

        alerts.into_iter().take(limit).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_traversal_and_risk_scoring() {
        let mut graph = SupplyChainGraph::empty();
        graph.load_builtin_graph();
        graph.load_builtin_events();

        // 1. Direct Tier-1 traversal for AAPL
        let alerts_depth_1 = graph.compute_risk_alerts("AAPL", 1, 30, 0.5, 10);
        assert!(!alerts_depth_1.is_empty());
        assert!(alerts_depth_1.iter().all(|a| a.depth == 1));
        assert!(alerts_depth_1.iter().any(|a| a.related_ticker == "TSM"));

        // 2. Tier-2 traversal for AAPL (includes ASML via TSM)
        let alerts_depth_2 = graph.compute_risk_alerts("AAPL", 2, 30, 0.3, 20);
        let has_asml = alerts_depth_2
            .iter()
            .any(|a| a.related_ticker == "ASML" && a.depth == 2);
        assert!(has_asml, "ASML should be discovered at depth 2 via TSM");

        // 3. Risk score ordering check
        for i in 1..alerts_depth_2.len() {
            assert!(alerts_depth_2[i - 1].risk_score >= alerts_depth_2[i].risk_score);
        }
    }
}
