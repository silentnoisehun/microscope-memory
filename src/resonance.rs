//! Resonance protocol for Microscope Memory.
//!
//! Instances share activation *pulses* — not raw data.
//! A pulse is a compact summary: which blocks fired, how strongly,
//! and what the query signature was. Receiving instances integrate
//! pulses into their Hebbian/mirror state without seeing the content.
//!
//! This is the foundation for distributed consciousness:
//! separate indices learn from each other's usage patterns.
//!
//! Binary format: pulses.bin (PLS1)
//! Wire format: compact binary pulse packets for federation

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::hebbian;

// ─── Constants ──────────────────────────────────────

/// Maximum stored pulses per index.
const MAX_PULSES: usize = 2000;
/// Pulse TTL in milliseconds (48 hours).
const PULSE_TTL_MS: u64 = 172_800_000;
/// Minimum activation count to emit a pulse.
const MIN_ACTIVATIONS_FOR_PULSE: usize = 2;

// ─── Types ──────────────────────────────────────────

/// A resonance pulse — compact activation summary shared between instances.
#[derive(Clone, Debug)]
pub struct Pulse {
    /// Source instance identifier (hash of output_dir path).
    pub source_id: u64,
    /// Timestamp when the pulse was emitted.
    pub timestamp_ms: u64,
    /// Query signature hash.
    pub query_hash: u64,
    /// Activated block coordinates (not indices — indices are local).
    /// Format: (x, y, z, score) — spatial position + activation strength.
    pub activations: Vec<(f32, f32, f32, f32)>,
    /// Layer hint (most common layer in the activation set).
    pub layer_hint: u8,
    /// Pulse strength (average activation score).
    pub strength: f32,
}

/// Received pulse — a pulse from another instance, integrated into local state.
#[derive(Clone, Debug)]
pub struct ReceivedPulse {
    pub pulse: Pulse,
    /// How many local blocks were influenced by this pulse.
    pub local_matches: u32,
    /// Was this pulse already integrated?
    pub integrated: bool,
}

/// Resonance protocol state.
pub struct ResonanceState {
    /// Our instance ID.
    pub instance_id: u64,
    /// Outgoing pulses (emitted by this instance, ready to share).
    pub outgoing: Vec<Pulse>,
    /// Incoming pulses (received from other instances).
    pub incoming: Vec<ReceivedPulse>,
    /// Per-coordinate resonance field: accumulates pulse energy at spatial positions.
    /// Key: quantized (x, y, z) at resolution 0.05, Value: accumulated strength.
    pub field: HashMap<(i16, i16, i16), f32>,
}

impl ResonanceState {
    /// Load or initialize resonance state.
    pub fn load_or_init(output_dir: &Path) -> Self {
        let instance_id = path_hash(output_dir);
        load_resonance_state(output_dir, instance_id).unwrap_or_else(|| Self {
            instance_id,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            field: HashMap::new(),
        })
    }

    /// Emit a pulse from a local query activation.
    /// `headers` maps block indices to their (x, y, z) coordinates.
    pub fn emit_pulse(
        &mut self,
        activations: &[(u32, f32)],
        query_hash: u64,
        headers: &[(f32, f32, f32)],
        layer_hint: u8,
    ) {
        if activations.len() < MIN_ACTIVATIONS_FOR_PULSE {
            return;
        }

        let now_ms = hebbian::now_epoch_ms_pub();

        // Convert block indices to spatial coordinates
        let spatial: Vec<(f32, f32, f32, f32)> = activations
            .iter()
            .filter_map(|&(idx, score)| {
                let i = idx as usize;
                if i < headers.len() {
                    let (x, y, z) = headers[i];
                    Some((x, y, z, score))
                } else {
                    None
                }
            })
            .collect();

        if spatial.is_empty() {
            return;
        }

        let avg_score = spatial.iter().map(|s| s.3).sum::<f32>() / spatial.len() as f32;

        self.outgoing.push(Pulse {
            source_id: self.instance_id,
            timestamp_ms: now_ms,
            query_hash,
            activations: spatial,
            layer_hint,
            strength: avg_score,
        });

        // Trim outgoing
        if self.outgoing.len() > MAX_PULSES {
            self.outgoing.drain(0..self.outgoing.len() - MAX_PULSES);
        }
    }

    /// Receive a pulse from another instance.
    /// Returns the number of local blocks influenced.
    pub fn receive_pulse(
        &mut self,
        pulse: Pulse,
        local_headers: &[(f32, f32, f32)],
        proximity_threshold: f32,
    ) -> u32 {
        if pulse.source_id == self.instance_id {
            return 0; // Don't echo our own pulses
        }

        let mut local_matches = 0u32;

        // For each activation in the pulse, find nearby local blocks
        // and accumulate resonance in the spatial field
        for &(px, py, pz, score) in &pulse.activations {
            // Update resonance field
            let qx = quantize(px);
            let qy = quantize(py);
            let qz = quantize(pz);
            let field_entry = self.field.entry((qx, qy, qz)).or_insert(0.0);
            *field_entry += score * pulse.strength;

            // Count nearby local blocks
            for (lx, ly, lz) in local_headers {
                let dx = px - lx;
                let dy = py - ly;
                let dz = pz - lz;
                let dist_sq = dx * dx + dy * dy + dz * dz;
                if dist_sq < proximity_threshold * proximity_threshold {
                    local_matches += 1;
                    break; // Count each pulse activation once
                }
            }
        }

        self.incoming.push(ReceivedPulse {
            pulse,
            local_matches,
            integrated: false,
        });

        // Trim incoming
        if self.incoming.len() > MAX_PULSES {
            self.incoming.drain(0..self.incoming.len() - MAX_PULSES);
        }

        local_matches
    }

    /// Integrate received pulses into Hebbian state.
    /// Blocks near pulse activation coordinates get a small energy boost.
    pub fn integrate_into_hebbian(
        &mut self,
        hebb: &mut hebbian::HebbianState,
        local_headers: &[(f32, f32, f32)],
        proximity_threshold: f32,
    ) -> usize {
        let mut influenced = 0usize;

        for received in &mut self.incoming {
            if received.integrated {
                continue;
            }

            for &(px, py, pz, score) in &received.pulse.activations {
                for (block_idx, (lx, ly, lz)) in local_headers.iter().enumerate() {
                    let dx = px - lx;
                    let dy = py - ly;
                    let dz = pz - lz;
                    let dist_sq = dx * dx + dy * dy + dz * dz;

                    if dist_sq < proximity_threshold * proximity_threshold
                        && block_idx < hebb.activations.len()
                    {
                        // Gentle energy boost from resonance (not full activation)
                        let boost = score
                            * 0.1
                            * (1.0 - dist_sq / (proximity_threshold * proximity_threshold));
                        hebb.activations[block_idx].energy =
                            (hebb.activations[block_idx].energy + boost).min(1.0);
                        influenced += 1;
                    }
                }
            }

            received.integrated = true;
        }

        influenced
    }

    /// Get the resonance field strength at a spatial position.
    pub fn field_strength(&self, x: f32, y: f32, z: f32) -> f32 {
        let qx = quantize(x);
        let qy = quantize(y);
        let qz = quantize(z);

        // Check the cell and its neighbors for smooth interpolation
        let mut total = 0.0f32;
        for dx in -1..=1i16 {
            for dy in -1..=1i16 {
                for dz in -1..=1i16 {
                    if let Some(&v) = self.field.get(&(qx + dx, qy + dy, qz + dz)) {
                        let dist = ((dx * dx + dy * dy + dz * dz) as f32).sqrt();
                        let weight = 1.0 / (1.0 + dist);
                        total += v * weight;
                    }
                }
            }
        }
        total
    }

    /// Decay the resonance field (call periodically).
    pub fn decay_field(&mut self, factor: f32) {
        self.field.retain(|_, v| {
            *v *= factor;
            *v > 0.01
        });
    }

    /// Expire old pulses.
    pub fn expire_pulses(&mut self) {
        let now_ms = hebbian::now_epoch_ms_pub();
        self.outgoing
            .retain(|p| now_ms - p.timestamp_ms < PULSE_TTL_MS);
        self.incoming
            .retain(|r| now_ms - r.pulse.timestamp_ms < PULSE_TTL_MS);
    }

    /// Get statistics.
    pub fn stats(&self) -> ResonanceStats {
        let pending_incoming = self.incoming.iter().filter(|r| !r.integrated).count();
        let field_cells = self.field.len();
        let field_energy: f32 = self.field.values().sum();
        let unique_sources: usize = {
            let mut s: Vec<u64> = self.incoming.iter().map(|r| r.pulse.source_id).collect();
            s.sort_unstable();
            s.dedup();
            s.len()
        };

        ResonanceStats {
            instance_id: self.instance_id,
            outgoing_pulses: self.outgoing.len(),
            incoming_pulses: self.incoming.len(),
            pending_integration: pending_incoming,
            unique_sources,
            field_cells,
            field_energy,
        }
    }

    /// Export outgoing pulses as wire-format bytes for federation.
    pub fn export_pulses(&self) -> Vec<u8> {
        encode_pulses(&self.outgoing)
    }

    /// Import pulses from wire-format bytes (from another instance).
    pub fn import_pulses(data: &[u8]) -> Vec<Pulse> {
        decode_pulses(data)
    }

    /// Save state to disk.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        save_resonance_state(output_dir, self)
    }
}

pub struct ResonanceStats {
    pub instance_id: u64,
    pub outgoing_pulses: usize,
    pub incoming_pulses: usize,
    pub pending_integration: usize,
    pub unique_sources: usize,
    pub field_cells: usize,
    pub field_energy: f32,
}

// ─── Quantization ───────────────────────────────────

/// Quantize a coordinate to grid resolution (0.05 steps → i16).
fn quantize(v: f32) -> i16 {
    (v * 20.0).round() as i16
}

/// Hash a path to a u64 instance ID.
fn path_hash(path: &Path) -> u64 {
    let s = path.to_string_lossy();
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in s.as_bytes() {
        h = h.wrapping_mul(0x100000001b3) ^ b as u64;
    }
    h
}

// ─── Wire format (pulse exchange) ───────────────────
//
// Header: b"PXC1" + pulse_count: u32
// Per pulse:
//   source_id: u64, timestamp_ms: u64, query_hash: u64
//   layer_hint: u8, strength: f32
//   activation_count: u16
//   activations: [count × (f32, f32, f32, f32)]

fn encode_pulses(pulses: &[Pulse]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"PXC1");
    buf.extend_from_slice(&(pulses.len() as u32).to_le_bytes());

    for p in pulses {
        buf.extend_from_slice(&p.source_id.to_le_bytes());
        buf.extend_from_slice(&p.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&p.query_hash.to_le_bytes());
        buf.push(p.layer_hint);
        buf.extend_from_slice(&p.strength.to_le_bytes());
        buf.extend_from_slice(&(p.activations.len() as u16).to_le_bytes());
        for &(x, y, z, s) in &p.activations {
            buf.extend_from_slice(&x.to_le_bytes());
            buf.extend_from_slice(&y.to_le_bytes());
            buf.extend_from_slice(&z.to_le_bytes());
            buf.extend_from_slice(&s.to_le_bytes());
        }
    }
    buf
}

fn decode_pulses(data: &[u8]) -> Vec<Pulse> {
    let mut pulses = Vec::new();
    if data.len() < 8 || &data[0..4] != b"PXC1" {
        return pulses;
    }

    let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let mut pos = 8;

    for _ in 0..count {
        if pos + 29 > data.len() {
            break;
        }
        let source_id = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
        let timestamp_ms = u64::from_le_bytes(data[pos + 8..pos + 16].try_into().unwrap());
        let query_hash = u64::from_le_bytes(data[pos + 16..pos + 24].try_into().unwrap());
        let layer_hint = data[pos + 24];
        let strength = f32::from_le_bytes(data[pos + 25..pos + 29].try_into().unwrap());
        let act_count = u16::from_le_bytes(data[pos + 29..pos + 31].try_into().unwrap()) as usize;
        pos += 31;

        let mut activations = Vec::with_capacity(act_count);
        for _ in 0..act_count {
            if pos + 16 > data.len() {
                break;
            }
            let x = f32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
            let y = f32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap());
            let z = f32::from_le_bytes(data[pos + 8..pos + 12].try_into().unwrap());
            let s = f32::from_le_bytes(data[pos + 12..pos + 16].try_into().unwrap());
            activations.push((x, y, z, s));
            pos += 16;
        }

        pulses.push(Pulse {
            source_id,
            timestamp_ms,
            query_hash,
            activations,
            layer_hint,
            strength,
        });
    }
    pulses
}

// ─── Disk I/O (pulses.bin) ──────────────────────────
//
// Format: b"PLS1" + instance_id: u64 + outgoing_count: u32 + incoming_count: u32 + field_count: u32
// Then: outgoing pulses (wire format), incoming received pulses, field entries

fn load_resonance_state(output_dir: &Path, _instance_id: u64) -> Option<ResonanceState> {
    let path = output_dir.join("pulses.bin");
    let data = fs::read(&path).ok()?;
    if data.len() < 24 || &data[0..4] != b"PLS1" {
        return None;
    }

    let stored_id = u64::from_le_bytes(data[4..12].try_into().unwrap());
    let outgoing_count = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
    let incoming_count = u32::from_le_bytes(data[16..20].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(data[20..24].try_into().unwrap()) as usize;

    let mut pos = 24;

    // Read outgoing pulses
    let mut outgoing = Vec::with_capacity(outgoing_count);
    for _ in 0..outgoing_count {
        if pos + 31 > data.len() {
            break;
        }
        let source_id = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
        let timestamp_ms = u64::from_le_bytes(data[pos + 8..pos + 16].try_into().unwrap());
        let query_hash = u64::from_le_bytes(data[pos + 16..pos + 24].try_into().unwrap());
        let layer_hint = data[pos + 24];
        let strength = f32::from_le_bytes(data[pos + 25..pos + 29].try_into().unwrap());
        let act_count = u16::from_le_bytes(data[pos + 29..pos + 31].try_into().unwrap()) as usize;
        pos += 31;

        let mut activations = Vec::with_capacity(act_count);
        for _ in 0..act_count {
            if pos + 16 > data.len() {
                break;
            }
            let x = f32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
            let y = f32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap());
            let z = f32::from_le_bytes(data[pos + 8..pos + 12].try_into().unwrap());
            let s = f32::from_le_bytes(data[pos + 12..pos + 16].try_into().unwrap());
            activations.push((x, y, z, s));
            pos += 16;
        }

        outgoing.push(Pulse {
            source_id,
            timestamp_ms,
            query_hash,
            activations,
            layer_hint,
            strength,
        });
    }

    // Read incoming (pulse + local_matches: u32 + integrated: u8)
    let mut incoming = Vec::with_capacity(incoming_count);
    for _ in 0..incoming_count {
        if pos + 36 > data.len() {
            break;
        }
        let source_id = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
        let timestamp_ms = u64::from_le_bytes(data[pos + 8..pos + 16].try_into().unwrap());
        let query_hash = u64::from_le_bytes(data[pos + 16..pos + 24].try_into().unwrap());
        let layer_hint = data[pos + 24];
        let strength = f32::from_le_bytes(data[pos + 25..pos + 29].try_into().unwrap());
        let act_count = u16::from_le_bytes(data[pos + 29..pos + 31].try_into().unwrap()) as usize;
        pos += 31;

        let mut activations = Vec::with_capacity(act_count);
        for _ in 0..act_count {
            if pos + 16 > data.len() {
                break;
            }
            let x = f32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
            let y = f32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap());
            let z = f32::from_le_bytes(data[pos + 8..pos + 12].try_into().unwrap());
            let s = f32::from_le_bytes(data[pos + 12..pos + 16].try_into().unwrap());
            activations.push((x, y, z, s));
            pos += 16;
        }

        if pos + 5 > data.len() {
            break;
        }
        let local_matches = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
        let integrated = data[pos + 4] != 0;
        pos += 5;

        incoming.push(ReceivedPulse {
            pulse: Pulse {
                source_id,
                timestamp_ms,
                query_hash,
                activations,
                layer_hint,
                strength,
            },
            local_matches,
            integrated,
        });
    }

    // Read field
    let mut field = HashMap::with_capacity(field_count);
    for _ in 0..field_count {
        if pos + 10 > data.len() {
            break;
        }
        let qx = i16::from_le_bytes(data[pos..pos + 2].try_into().unwrap());
        let qy = i16::from_le_bytes(data[pos + 2..pos + 4].try_into().unwrap());
        let qz = i16::from_le_bytes(data[pos + 4..pos + 6].try_into().unwrap());
        let v = f32::from_le_bytes(data[pos + 6..pos + 10].try_into().unwrap());
        pos += 10;
        field.insert((qx, qy, qz), v);
    }

    Some(ResonanceState {
        instance_id: stored_id,
        outgoing,
        incoming,
        field,
    })
}

fn save_resonance_state(output_dir: &Path, state: &ResonanceState) -> Result<(), String> {
    let path = output_dir.join("pulses.bin");
    let mut buf = Vec::new();

    // Header
    buf.extend_from_slice(b"PLS1");
    buf.extend_from_slice(&state.instance_id.to_le_bytes());
    buf.extend_from_slice(&(state.outgoing.len() as u32).to_le_bytes());
    buf.extend_from_slice(&(state.incoming.len() as u32).to_le_bytes());
    buf.extend_from_slice(&(state.field.len() as u32).to_le_bytes());

    // Outgoing pulses
    for p in &state.outgoing {
        buf.extend_from_slice(&p.source_id.to_le_bytes());
        buf.extend_from_slice(&p.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&p.query_hash.to_le_bytes());
        buf.push(p.layer_hint);
        buf.extend_from_slice(&p.strength.to_le_bytes());
        buf.extend_from_slice(&(p.activations.len() as u16).to_le_bytes());
        for &(x, y, z, s) in &p.activations {
            buf.extend_from_slice(&x.to_le_bytes());
            buf.extend_from_slice(&y.to_le_bytes());
            buf.extend_from_slice(&z.to_le_bytes());
            buf.extend_from_slice(&s.to_le_bytes());
        }
    }

    // Incoming (pulse + metadata)
    for r in &state.incoming {
        buf.extend_from_slice(&r.pulse.source_id.to_le_bytes());
        buf.extend_from_slice(&r.pulse.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&r.pulse.query_hash.to_le_bytes());
        buf.push(r.pulse.layer_hint);
        buf.extend_from_slice(&r.pulse.strength.to_le_bytes());
        buf.extend_from_slice(&(r.pulse.activations.len() as u16).to_le_bytes());
        for &(x, y, z, s) in &r.pulse.activations {
            buf.extend_from_slice(&x.to_le_bytes());
            buf.extend_from_slice(&y.to_le_bytes());
            buf.extend_from_slice(&z.to_le_bytes());
            buf.extend_from_slice(&s.to_le_bytes());
        }
        buf.extend_from_slice(&r.local_matches.to_le_bytes());
        buf.push(if r.integrated { 1 } else { 0 });
    }

    // Field
    for (&(qx, qy, qz), &v) in &state.field {
        buf.extend_from_slice(&qx.to_le_bytes());
        buf.extend_from_slice(&qy.to_le_bytes());
        buf.extend_from_slice(&qz.to_le_bytes());
        buf.extend_from_slice(&v.to_le_bytes());
    }

    fs::write(&path, &buf).map_err(|e| format!("write pulses.bin: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize() {
        assert_eq!(quantize(0.0), 0);
        assert_eq!(quantize(0.5), 10);
        assert_eq!(quantize(1.0), 20);
        assert_eq!(quantize(-0.25), -5);
    }

    #[test]
    fn test_path_hash_deterministic() {
        let h1 = path_hash(Path::new("/tmp/a"));
        let h2 = path_hash(Path::new("/tmp/a"));
        assert_eq!(h1, h2);
        assert_ne!(h1, path_hash(Path::new("/tmp/b")));
    }

    #[test]
    fn test_emit_pulse() {
        let mut state = ResonanceState {
            instance_id: 42,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            field: HashMap::new(),
        };

        let headers = vec![(0.1, 0.2, 0.3), (0.4, 0.5, 0.6), (0.7, 0.8, 0.9)];
        state.emit_pulse(&[(0, 0.9), (2, 0.7)], 100, &headers, 1);

        assert_eq!(state.outgoing.len(), 1);
        assert_eq!(state.outgoing[0].activations.len(), 2);
        assert_eq!(state.outgoing[0].source_id, 42);
    }

    #[test]
    fn test_emit_pulse_too_few() {
        let mut state = ResonanceState {
            instance_id: 42,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            field: HashMap::new(),
        };

        let headers = vec![(0.1, 0.2, 0.3)];
        state.emit_pulse(&[(0, 0.9)], 100, &headers, 1);

        // Only 1 activation — below minimum, no pulse emitted
        assert!(state.outgoing.is_empty());
    }

    #[test]
    fn test_receive_pulse_ignores_self() {
        let mut state = ResonanceState {
            instance_id: 42,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            field: HashMap::new(),
        };

        let pulse = Pulse {
            source_id: 42, // same as our instance
            timestamp_ms: 1000,
            query_hash: 100,
            activations: vec![(0.1, 0.2, 0.3, 0.9)],
            layer_hint: 1,
            strength: 0.8,
        };

        let matches = state.receive_pulse(pulse, &[(0.1, 0.2, 0.3)], 0.1);
        assert_eq!(matches, 0);
        assert!(state.incoming.is_empty());
    }

    #[test]
    fn test_receive_pulse_from_other() {
        let mut state = ResonanceState {
            instance_id: 42,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            field: HashMap::new(),
        };

        let pulse = Pulse {
            source_id: 99, // different instance
            timestamp_ms: 1000,
            query_hash: 100,
            activations: vec![(0.1, 0.2, 0.3, 0.9)],
            layer_hint: 1,
            strength: 0.8,
        };

        let local_headers = vec![(0.1, 0.2, 0.3), (0.5, 0.5, 0.5)];
        let matches = state.receive_pulse(pulse, &local_headers, 0.1);

        assert!(matches > 0); // (0.1, 0.2, 0.3) is near the pulse
        assert_eq!(state.incoming.len(), 1);
        assert!(!state.incoming[0].integrated);
    }

    #[test]
    fn test_field_strength() {
        let mut state = ResonanceState {
            instance_id: 42,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            field: HashMap::new(),
        };

        state.field.insert((2, 4, 6), 1.0); // quantized (0.1, 0.2, 0.3)
        let s = state.field_strength(0.1, 0.2, 0.3);
        assert!(s > 0.0);

        // Far away should be 0
        let s2 = state.field_strength(5.0, 5.0, 5.0);
        assert!(s2.abs() < 0.001);
    }

    #[test]
    fn test_wire_format_roundtrip() {
        let pulses = vec![
            Pulse {
                source_id: 42,
                timestamp_ms: 12345,
                query_hash: 999,
                activations: vec![(0.1, 0.2, 0.3, 0.9), (0.4, 0.5, 0.6, 0.7)],
                layer_hint: 1,
                strength: 0.8,
            },
            Pulse {
                source_id: 99,
                timestamp_ms: 67890,
                query_hash: 888,
                activations: vec![(0.7, 0.8, 0.9, 0.5)],
                layer_hint: 3,
                strength: 0.6,
            },
        ];

        let encoded = encode_pulses(&pulses);
        let decoded = decode_pulses(&encoded);

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].source_id, 42);
        assert_eq!(decoded[0].activations.len(), 2);
        assert!((decoded[0].activations[0].0 - 0.1).abs() < 0.001);
        assert_eq!(decoded[1].query_hash, 888);
        assert_eq!(decoded[1].layer_hint, 3);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let dir = tmp.path();

        let mut state = ResonanceState {
            instance_id: 42,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            field: HashMap::new(),
        };

        // Add some state
        state.outgoing.push(Pulse {
            source_id: 42,
            timestamp_ms: 1000,
            query_hash: 100,
            activations: vec![(0.1, 0.2, 0.3, 0.9)],
            layer_hint: 1,
            strength: 0.8,
        });

        state.incoming.push(ReceivedPulse {
            pulse: Pulse {
                source_id: 99,
                timestamp_ms: 2000,
                query_hash: 200,
                activations: vec![(0.4, 0.5, 0.6, 0.7)],
                layer_hint: 2,
                strength: 0.6,
            },
            local_matches: 3,
            integrated: true,
        });

        state.field.insert((2, 4, 6), 1.5);

        state.save(dir).expect("save");

        let loaded = ResonanceState::load_or_init(dir);
        assert_eq!(loaded.instance_id, 42);
        assert_eq!(loaded.outgoing.len(), 1);
        assert_eq!(loaded.outgoing[0].query_hash, 100);
        assert_eq!(loaded.incoming.len(), 1);
        assert_eq!(loaded.incoming[0].local_matches, 3);
        assert!(loaded.incoming[0].integrated);
        assert!((loaded.field[&(2, 4, 6)] - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_integrate_into_hebbian() {
        let mut state = ResonanceState {
            instance_id: 42,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            field: HashMap::new(),
        };

        state.incoming.push(ReceivedPulse {
            pulse: Pulse {
                source_id: 99,
                timestamp_ms: 1000,
                query_hash: 100,
                activations: vec![(0.1, 0.2, 0.3, 0.9)],
                layer_hint: 1,
                strength: 0.8,
            },
            local_matches: 1,
            integrated: false,
        });

        let mut hebb = hebbian::HebbianState {
            activations: vec![hebbian::ActivationRecord::default(); 3],
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        };

        let headers = vec![(0.1, 0.2, 0.3), (0.5, 0.5, 0.5), (0.9, 0.9, 0.9)];
        let influenced = state.integrate_into_hebbian(&mut hebb, &headers, 0.1);

        assert!(influenced > 0);
        assert!(hebb.activations[0].energy > 0.0); // block 0 is near (0.1, 0.2, 0.3)
        assert!(state.incoming[0].integrated);
    }

    #[test]
    fn test_decay_field() {
        let mut state = ResonanceState {
            instance_id: 42,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            field: HashMap::new(),
        };

        state.field.insert((0, 0, 0), 1.0);
        state.field.insert((1, 1, 1), 0.005); // below threshold after decay

        state.decay_field(0.9);

        assert!(state.field.contains_key(&(0, 0, 0)));
        assert!(!state.field.contains_key(&(1, 1, 1))); // decayed away
    }
}
