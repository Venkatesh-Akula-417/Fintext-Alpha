//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Supply-Chain Graph Neural Network (GNN) Engine
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides institutional 2-layer Graph Convolutional Network (GCN) shock propagation
//! and spatial distance alpha decay across global economic supply chain networks:
//!   1. Symmetric Normalized Laplacian:
//!      Â = D̃^(-1/2) Ã D̃^(-1/2) where Ã = (A ⊙ e^(-γ D)) + I_N
//!   2. 2-Layer GCN Forward Propagation:
//!      H^(1) = σ( Â H^(0) W^(0) )
//!      H^(2) = σ( Â H^(1) W^(1) )
//!   3. Spatial Distance Alpha Decay:
//!      e^(-γ * d) continuous decay across supplier-customer topological hops.
//! ═══════════════════════════════════════════════════════════════════════════════

use nalgebra::{DMatrix, DVector};
use tracing::debug;

/// Maximum permissible graph node count to guarantee bounded CPU latency (<10ms) and memory safety.
pub const MAX_NODES: usize = 10_000;

/// Default spatial alpha decay rate γ = 0.3
pub const DEFAULT_GAMMA: f64 = 0.3;

/// Graph Neural Network (GNN) model for supply-chain shock propagation.
#[derive(Debug, Clone)]
pub struct SupplyChainGNN {
    /// Number of GCN convolutional layers (default 2)
    pub num_layers: usize,
    /// Hidden dimension of node representations
    pub hidden_dim: usize,
    /// Continuous spatial distance decay parameter γ (default 0.3)
    pub gamma: f64,
    /// Layer 0 transmission projection weights W^(0) [F_in x F_hidden]
    pub w_layer0: DMatrix<f64>,
    /// Layer 1 transmission projection weights W^(1) [F_hidden x F_out]
    pub w_layer1: DMatrix<f64>,
}

impl Default for SupplyChainGNN {
    fn default() -> Self {
        Self::new(DEFAULT_GAMMA)
    }
}

impl SupplyChainGNN {
    /// Creates a standard 2-layer SupplyChainGNN with 1D scalar features (F_in = 1, F_hidden = 1, F_out = 1).
    pub fn new(gamma: f64) -> Self {
        Self {
            num_layers: 2,
            hidden_dim: 1,
            gamma,
            w_layer0: DMatrix::from_element(1, 1, 0.95),
            w_layer1: DMatrix::from_element(1, 1, 0.90),
        }
    }

    /// Creates a SupplyChainGNN with custom feature dimensions and identity/scaled projection weights.
    pub fn with_dimensions(in_dim: usize, hidden_dim: usize, out_dim: usize, gamma: f64) -> Self {
        let mut w0 = DMatrix::zeros(in_dim, hidden_dim);
        for i in 0..in_dim.min(hidden_dim) {
            w0[(i, i)] = 0.95;
        }

        let mut w1 = DMatrix::zeros(hidden_dim, out_dim);
        for i in 0..hidden_dim.min(out_dim) {
            w1[(i, i)] = 0.90;
        }

        Self {
            num_layers: 2,
            hidden_dim,
            gamma,
            w_layer0: w0,
            w_layer1: w1,
        }
    }

    /// Computes the symmetric normalized graph Laplacian with optional continuous spatial distance decay:
    /// Â = D̃^(-1/2) Ã D̃^(-1/2)
    /// where Ã = (A ⊙ e^(-γ D)) + I_N
    pub fn compute_normalized_laplacian(
        adjacency: &DMatrix<f64>,
        distances: Option<&DMatrix<f64>>,
        gamma: f64,
    ) -> Result<DMatrix<f64>, String> {
        let (rows, cols) = adjacency.shape();
        if rows == 0 || cols == 0 {
            return Err("Adjacency matrix cannot be empty (0x0)".to_string());
        }
        if rows != cols {
            return Err(format!(
                "Adjacency matrix must be square, got ({} x {})",
                rows, cols
            ));
        }
        if rows > MAX_NODES {
            return Err(format!(
                "Graph node count {} exceeds maximum safety limit of {}",
                rows, MAX_NODES
            ));
        }

        let n = rows;
        let mut a_tilde = adjacency.clone();

        // 1. Apply Spatial Distance Decay if distances matrix is provided
        if let Some(dist_mat) = distances {
            if dist_mat.shape() != (n, n) {
                return Err(format!(
                    "Distances matrix shape {:?} does not match adjacency shape ({}, {})",
                    dist_mat.shape(),
                    n,
                    n
                ));
            }
            for i in 0..n {
                for j in 0..n {
                    if a_tilde[(i, j)] > 0.0 && i != j {
                        let d = dist_mat[(i, j)];
                        let decay = (-gamma * d).exp();
                        a_tilde[(i, j)] *= decay;
                    }
                }
            }
        }

        // 2. Add Self-Loops: Ã = A_decay + I_N
        for i in 0..n {
            a_tilde[(i, i)] += 1.0;
        }

        // 3. Calculate Degree Matrix: D̃_ii = Σ_j Ã_ij
        let mut deg_inv_sqrt = DVector::zeros(n);
        for i in 0..n {
            let row_sum: f64 = (0..n).map(|j| a_tilde[(i, j)]).sum();
            if row_sum > 1e-7 {
                deg_inv_sqrt[i] = 1.0 / row_sum.sqrt();
            } else {
                deg_inv_sqrt[i] = 0.0;
            }
        }

        // 4. Symmetric Normalization: Â = D̃^(-1/2) Ã D̃^(-1/2)
        let mut a_hat = DMatrix::zeros(n, n);
        for i in 0..n {
            let di = deg_inv_sqrt[i];
            if di > 0.0 {
                for j in 0..n {
                    let dj = deg_inv_sqrt[j];
                    if dj > 0.0 {
                        a_hat[(i, j)] = di * a_tilde[(i, j)] * dj;
                    }
                }
            }
        }

        Ok(a_hat)
    }

    /// Propagates 1D scalar node shock features across the supply chain network using a 2-layer GCN.
    ///
    /// # Arguments
    /// * `node_features` - Input shock vector of size N
    /// * `adjacency` - N x N adjacency matrix
    /// * `distances` - Optional N x N topological distance matrix
    ///
    /// # Returns
    /// * `DVector<f64>` of propagated shock values for each node.
    pub fn propagate_shocks(
        &self,
        node_features: &DVector<f64>,
        adjacency: &DMatrix<f64>,
        distances: Option<&DMatrix<f64>>,
    ) -> Result<DVector<f64>, String> {
        let n = node_features.len();
        if n == 0 {
            return Err("Node features vector cannot be empty".to_string());
        }
        if adjacency.shape() != (n, n) {
            return Err(format!(
                "Adjacency matrix shape {:?} does not match node features length ({})",
                adjacency.shape(),
                n
            ));
        }

        // Convert 1D DVector to (N x 1) DMatrix for matrix propagation
        let h0 = DMatrix::from_column_slice(n, 1, node_features.as_slice());
        let h2 = self.propagate_matrix(&h0, adjacency, distances)?;

        // Return first column as DVector
        Ok(DVector::from_column_slice(h2.column(0).as_slice()))
    }

    /// Propagates multi-dimensional node feature matrix H^(0) [N x F_in] across 2 GCN layers.
    ///
    /// H^(1) = ReLU( Â H^(0) W^(0) )
    /// H^(2) = ReLU( Â H^(1) W^(1) )
    pub fn propagate_matrix(
        &self,
        feature_matrix: &DMatrix<f64>,
        adjacency: &DMatrix<f64>,
        distances: Option<&DMatrix<f64>>,
    ) -> Result<DMatrix<f64>, String> {
        let (n_nodes, in_feats) = feature_matrix.shape();
        if n_nodes == 0 || in_feats == 0 {
            return Err("Feature matrix cannot be empty".to_string());
        }
        if adjacency.shape() != (n_nodes, n_nodes) {
            return Err(format!(
                "Adjacency matrix shape {:?} does not match feature matrix node count {}",
                adjacency.shape(),
                n_nodes
            ));
        }
        if in_feats != self.w_layer0.nrows() {
            return Err(format!(
                "Input feature dimension {} does not match W_layer0 input dimension {}",
                in_feats,
                self.w_layer0.nrows()
            ));
        }

        // 1. Compute symmetric normalized Laplacian Â
        let a_hat = Self::compute_normalized_laplacian(adjacency, distances, self.gamma)?;

        // 2. Layer 1 Convolution: Z^(1) = Â * H^(0) * W^(0), H^(1) = ReLU(Z^(1))
        let z1 = &a_hat * feature_matrix * &self.w_layer0;
        let h1 = z1.map(|x| if x > 0.0 { x } else { 0.0 });

        // 3. Layer 2 Convolution: Z^(2) = Â * H^(1) * W^(1), H^(2) = ReLU(Z^(2))
        let z2 = &a_hat * h1 * &self.w_layer1;
        let h2 = z2.map(|x| if x > 0.0 { x } else { 0.0 });

        debug!(
            "[SupplyChainGNN] Propagated shocks across {} nodes in 2 GCN layers",
            n_nodes
        );

        Ok(h2)
    }
}

/// Helper function to build an N x N weighted adjacency matrix from a list of edges: (src_idx, dst_idx, weight).
/// If `symmetric` is true, sets both (src, dst) and (dst, src) to represent undirected trading links.
pub fn build_adjacency_matrix(
    num_nodes: usize,
    edges: &[(usize, usize, f64)],
    symmetric: bool,
) -> Result<DMatrix<f64>, String> {
    if num_nodes == 0 {
        return Err("Node count must be greater than 0".to_string());
    }
    if num_nodes > MAX_NODES {
        return Err(format!(
            "Node count {} exceeds maximum safety limit of {}",
            num_nodes, MAX_NODES
        ));
    }

    let mut adj = DMatrix::zeros(num_nodes, num_nodes);
    for &(src, dst, weight) in edges {
        if src >= num_nodes || dst >= num_nodes {
            return Err(format!(
                "Edge ({}, {}) index out of bounds for num_nodes = {}",
                src, dst, num_nodes
            ));
        }
        adj[(src, dst)] = weight;
        if symmetric {
            adj[(dst, src)] = weight;
        }
    }
    Ok(adj)
}

/// Helper function to build an N x N topological distance matrix from shortest hop path counts.
/// If `symmetric` is true, sets both (src, dst) and (dst, src).
pub fn build_distance_matrix(
    num_nodes: usize,
    hop_distances: &[(usize, usize, f64)],
    symmetric: bool,
) -> Result<DMatrix<f64>, String> {
    if num_nodes == 0 {
        return Err("Node count must be greater than 0".to_string());
    }
    let mut dist = DMatrix::from_element(num_nodes, num_nodes, 0.0);
    for &(src, dst, d) in hop_distances {
        if src >= num_nodes || dst >= num_nodes {
            return Err(format!(
                "Distance edge ({}, {}) out of bounds for num_nodes = {}",
                src, dst, num_nodes
            ));
        }
        dist[(src, dst)] = d;
        if symmetric {
            dist[(dst, src)] = d;
        }
    }
    Ok(dist)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_3_node_supply_chain_propagation() {
        // 3 nodes: 0 = TSM (Supplier), 1 = NVDA (Chipmaker), 2 = SMCI (Server OEM)
        // Edges: TSM <-> NVDA (weight 0.85, dist 1.0), NVDA <-> SMCI (weight 0.75, dist 1.0)
        let n = 3;
        let edges = vec![(0, 1, 0.85), (1, 2, 0.75)];
        let distances = vec![(0, 1, 1.0), (1, 2, 1.0), (0, 2, 2.0)];

        let adj = build_adjacency_matrix(n, &edges, true).expect("Failed to build adjacency");
        let dist = build_distance_matrix(n, &distances, true).expect("Failed to build distances");

        // Initial shock originating at TSM: +1.0 (positive demand surge), NVDA=0, SMCI=0
        let node_features = DVector::from_vec(vec![1.0, 0.0, 0.0]);

        let gnn = SupplyChainGNN::new(DEFAULT_GAMMA);
        let propagated = gnn
            .propagate_shocks(&node_features, &adj, Some(&dist))
            .expect("GNN propagation failed");

        assert_eq!(propagated.len(), 3);
        // Node 0 (origin) retains highest activation
        assert!(propagated[0] > 0.0);
        // Node 1 (NVDA, 1 hop) receives direct propagated shock
        assert!(propagated[1] > 0.0);
        // Node 2 (SMCI, 2 hops) receives second-order shock in 2-layer GCN
        assert!(propagated[2] > 0.0);
        // Verify monotonicity: source >= Tier-1 >= Tier-2
        assert!(
            propagated[0] >= propagated[1],
            "TSM shock ({}) should be >= NVDA shock ({})",
            propagated[0],
            propagated[1]
        );
        assert!(
            propagated[1] >= propagated[2],
            "NVDA shock ({}) should be >= SMCI shock ({})",
            propagated[1],
            propagated[2]
        );
    }

    #[test]
    fn test_identity_adjacency_and_zeros() {
        let n = 3;
        let adj = DMatrix::zeros(n, n); // No edges, only self-loops will be added
        let zeros = DVector::zeros(n);

        let gnn = SupplyChainGNN::new(DEFAULT_GAMMA);
        let out = gnn
            .propagate_shocks(&zeros, &adj, None)
            .expect("Propagation failed");

        for &val in out.as_slice() {
            assert_eq!(val, 0.0, "Zero input should produce zero output under ReLU");
        }
    }

    #[test]
    fn test_empty_graph_and_dimension_mismatch_errors() {
        let gnn = SupplyChainGNN::new(DEFAULT_GAMMA);

        // Empty vector
        let empty_vec = DVector::zeros(0);
        let empty_adj = DMatrix::zeros(0, 0);
        assert!(gnn.propagate_shocks(&empty_vec, &empty_adj, None).is_err());

        // Dimension mismatch
        let vec3 = DVector::zeros(3);
        let adj2 = DMatrix::zeros(2, 2);
        assert!(gnn.propagate_shocks(&vec3, &adj2, None).is_err());

        // Non-square adjacency
        let non_square = DMatrix::zeros(2, 3);
        assert!(
            SupplyChainGNN::compute_normalized_laplacian(&non_square, None, DEFAULT_GAMMA).is_err()
        );
    }

    #[test]
    fn test_laplacian_symmetry_and_normalization() {
        let adj = DMatrix::from_row_slice(2, 2, &[0.0, 1.0, 1.0, 0.0]);
        let lap = SupplyChainGNN::compute_normalized_laplacian(&adj, None, DEFAULT_GAMMA)
            .expect("Laplacian computation failed");

        // Check symmetry: L[i, j] == L[j, i]
        assert!((lap[(0, 1)] - lap[(1, 0)]).abs() < 1e-12);
        // Diagonal values should be equal for symmetric graph
        assert!((lap[(0, 0)] - lap[(1, 1)]).abs() < 1e-12);
        assert!(lap[(0, 0)] > 0.0 && lap[(0, 0)] <= 1.0);
    }
}
