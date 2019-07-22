use prelude::*;

pub const PROVENANCE_DEPTH: usize = 3;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Updates {
    /// Whether updates should be stored or just max provenance
    store_updates: bool,
    /// Base provenance with all diffs applied
    max_provenance: Provenance,

    // The following fields are only used if we are storing updates
    /// Base provenance, including the label it represents
    min_provenance: Provenance,
    /// Provenance updates sent in outgoing packets
    updates: Vec<ProvenanceUpdate>,
}

impl Updates {
    pub fn init(&mut self, graph: &DomainGraph, store_updates: bool, root: ReplicaAddr) {
        self.store_updates = store_updates;
        for ni in graph.node_indices() {
            if graph[ni] == root {
                self.min_provenance.init(graph, root, ni, PROVENANCE_DEPTH);
                return;
            }
        }
        unreachable!();
    }

    pub fn init_in_domain(&mut self, shard: usize) -> AddrLabels {
        self.min_provenance.set_shard(shard);
        self.max_provenance = self.min_provenance.clone();
        self.max_provenance.into_addr_labels()
    }

    pub fn init_after_resume_at(&mut self, provenance: Provenance) -> AddrLabels {
        assert!(self.updates.is_empty());
        assert_eq!(self.min_provenance.label(), 0);
        assert_eq!(self.max_provenance.label(), 0);
        self.min_provenance = provenance;
        self.max_provenance = self.min_provenance.clone();
        self.max_provenance.into_addr_labels()
    }

    /// The label of the next message to send
    ///
    /// Replays don't get buffered and don't increment their label (they use the last label
    /// sent by this domain - think of replays as a snapshot of what's already been sent).
    pub fn next_label_to_send(&self, is_message: bool) -> usize {
        if is_message {
            self.max_provenance.label() + 1
        } else {
            self.max_provenance.label()
        }
    }

    /// Add the update of the next message we're about to send to our state
    pub fn add_update(&mut self, update: &ProvenanceUpdate) -> (AddrLabels, AddrLabels) {
        if self.store_updates {
            self.updates.push(update.clone());
        }
        self.max_provenance.apply_update(update)
    }

    /// Max provenance
    pub fn max(&self) -> &Provenance {
        &self.max_provenance
    }

    /// The provenance and updates that should be sent to ack a new incoming message
    pub fn ack_new_incoming(
        &self,
        incoming: ReplicaAddr,
    ) -> (Box<Provenance>, Vec<ProvenanceUpdate>) {
        if self.store_updates {
            let provenance = self.min_provenance.subgraph(incoming).unwrap().clone();
            let updates = self.updates
                .iter()
                .filter_map(|update| update.subgraph(incoming))
                .map(|update| *update.clone())
                .collect::<Vec<_>>();
            (provenance, updates)
        } else {
            assert!(self.updates.is_empty());
            let provenance = self.max_provenance.subgraph(incoming).unwrap().clone();
            (provenance, vec![])
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Payloads {
    /// Label of the last packet not in payloads
    min_label: usize,
    /// Packet payloads
    payloads: Vec<Box<Packet>>,
}

impl Payloads {
    /// Add the payload to our buffer
    pub fn add_payload(&mut self, m: Box<Packet>) {
        self.payloads.push(m);
    }

    /// Get payloads after this index, inclusive
    pub fn slice(&self, i: usize) -> Vec<Box<Packet>> {
        assert!(i >= self.min_label);
        if i == self.min_label {
            vec![]
        } else {
            let real_i = i - self.min_label - 1;
            self.payloads[real_i..]
                .iter()
                .map(|m| box m.clone_data())
                .collect()
        }
    }
}
