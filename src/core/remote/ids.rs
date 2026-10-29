//! Protobuf UUIDs on the wire.
//!
//! easytier-web serializes a `common.UUID` as four big-endian `u32` quarters, `{"part1":..,"part2":..,..}`, while
//! path parameters, map keys and `NetworkConfig.instance_id` use the canonical string. The app uses
//! [`uuid::Uuid`] everywhere and converts at the edge.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A protobuf `common.UUID` as easytier-web's JSON writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UuidQuad {
    /// Bytes 0..4, big-endian.
    pub part1: u32,
    /// Bytes 4..8, big-endian.
    pub part2: u32,
    /// Bytes 8..12, big-endian.
    pub part3: u32,
    /// Bytes 12..16, big-endian.
    pub part4: u32,
}

impl From<Uuid> for UuidQuad {
    fn from(id: Uuid) -> Self {
        let [
            a0,
            a1,
            a2,
            a3,
            b0,
            b1,
            b2,
            b3,
            c0,
            c1,
            c2,
            c3,
            d0,
            d1,
            d2,
            d3,
        ] = *id.as_bytes();
        Self {
            part1: u32::from_be_bytes([a0, a1, a2, a3]),
            part2: u32::from_be_bytes([b0, b1, b2, b3]),
            part3: u32::from_be_bytes([c0, c1, c2, c3]),
            part4: u32::from_be_bytes([d0, d1, d2, d3]),
        }
    }
}

impl From<UuidQuad> for Uuid {
    fn from(quad: UuidQuad) -> Self {
        let [a0, a1, a2, a3] = quad.part1.to_be_bytes();
        let [b0, b1, b2, b3] = quad.part2.to_be_bytes();
        let [c0, c1, c2, c3] = quad.part3.to_be_bytes();
        let [d0, d1, d2, d3] = quad.part4.to_be_bytes();
        Uuid::from_bytes([
            a0, a1, a2, a3, b0, b1, b2, b3, c0, c1, c2, c3, d0, d1, d2, d3,
        ])
    }
}

/// An id for a network instance the console is about to create.
///
/// Instance ids only have to be unique on one machine, they are not secret. Native builds use a random v4 UUID.
/// The web build has no random source, so it derives a v4-shaped id from the clock and a counter.
pub fn new_instance_id() -> Uuid {
    #[cfg(not(target_arch = "wasm32"))]
    {
        Uuid::new_v4()
    }
    #[cfg(target_arch = "wasm32")]
    {
        clock_instance_id()
    }
}

/// A v4-shaped id from the clock and a per-page counter (the web build's [`new_instance_id`]).
#[cfg(any(target_arch = "wasm32", test))]
fn clock_instance_id() -> Uuid {
    use std::cell::Cell;

    use sha2::{Digest, Sha256};

    thread_local! {
        static COUNTER: Cell<u64> = const { Cell::new(0) };
    }
    let count = COUNTER.with(|counter| {
        let next = counter.get().wrapping_add(1);
        counter.set(next);
        next
    });
    let mut hasher = Sha256::new();
    hasher.update(nsh_core::time::unix_ms().to_bits().to_be_bytes());
    hasher.update(count.to_be_bytes());
    hasher.update(b"easytierui-instance-id");
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    for (slot, byte) in bytes.iter_mut().zip(digest.iter()) {
        *slot = *byte;
    }
    uuid::Builder::from_random_bytes(bytes).into_uuid()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quads_round_trip_the_canonical_string() {
        let id: Uuid = "0b7d1d5e-3f55-4c0a-9a1e-77d2c40e19ab".parse().unwrap();
        let quad = UuidQuad::from(id);
        assert_eq!(quad.part1, 0x0B7D_1D5E);
        assert_eq!(quad.part2, 0x3F55_4C0A);
        assert_eq!(quad.part3, 0x9A1E_77D2);
        assert_eq!(quad.part4, 0xC40E_19AB);
        assert_eq!(Uuid::from(quad), id);
    }

    #[test]
    fn the_fixture_instance_id_is_part4_170() {
        let quad = UuidQuad {
            part4: 170,
            ..UuidQuad::default()
        };
        assert_eq!(
            Uuid::from(quad).to_string(),
            "00000000-0000-0000-0000-0000000000aa"
        );
    }

    #[test]
    fn clock_ids_are_valid_v4_and_distinct() {
        let a = clock_instance_id();
        let b = clock_instance_id();
        assert_ne!(a, b);
        assert_eq!(a.get_version_num(), 4);
    }

    #[test]
    fn fresh_instance_ids_are_distinct() {
        assert_ne!(new_instance_id(), new_instance_id());
    }
}
