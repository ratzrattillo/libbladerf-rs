//! Little-endian headers and zero-copy views of FPGA metadata messages.
//!
//! A timestamped SC16 USB buffer contains complete FPGA messages, each with
//! its own 16-byte header. Firmware 2.5+/FPGA 0.16+ use 4096-byte High-Speed
//! or 8192-byte SuperSpeed messages; older matched versions use 1024/2048.
//! Obtain the validated layout from an RX/TX stream or RF-link session.
//! Packet metadata uses a count of 32-bit payload words instead of fixed
//! timestamp-message framing.

use crate::{Error, Result, SemanticVersion};
use nusb::Speed;

/// Size of a metadata wire header in bytes.
pub const METADATA_HEADER_SIZE: usize = 16;

/// Decoded metadata header, independent of host byte order.
///
/// The first four bytes are reserved/diagnostic fields for SC16 timestamps,
/// or the payload word count, flags, and core ID for packet metadata.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MetadataHeader {
    reserved_or_length: u16,
    flags_or_core: u16,
    timestamp: u64,
    meta_flags: u32,
}

impl MetadataHeader {
    /// Creates a header from raw wire-field values.
    pub fn new(
        reserved_or_length: u16,
        flags_or_core: u16,
        timestamp: u64,
        meta_flags: u32,
    ) -> Self {
        Self {
            reserved_or_length,
            flags_or_core,
            timestamp,
            meta_flags,
        }
    }

    /// Decodes the first 16 bytes, returning `None` for truncated input.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let bytes = bytes.get(..METADATA_HEADER_SIZE)?;
        Some(Self {
            reserved_or_length: u16::from_le_bytes(bytes[..2].try_into().ok()?),
            flags_or_core: u16::from_le_bytes(bytes[2..4].try_into().ok()?),
            timestamp: u64::from_le_bytes(bytes[4..12].try_into().ok()?),
            meta_flags: u32::from_le_bytes(bytes[12..16].try_into().ok()?),
        })
    }

    /// Encodes all fields into a 16-byte little-endian wire header.
    pub fn to_bytes(self) -> [u8; METADATA_HEADER_SIZE] {
        let mut bytes = [0; METADATA_HEADER_SIZE];
        bytes[..2].copy_from_slice(&self.reserved_or_length.to_le_bytes());
        bytes[2..4].copy_from_slice(&self.flags_or_core.to_le_bytes());
        bytes[4..12].copy_from_slice(&self.timestamp.to_le_bytes());
        bytes[12..].copy_from_slice(&self.meta_flags.to_le_bytes());
        bytes
    }

    /// Returns the full 64-bit wire timestamp in sample-clock ticks.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Returns raw FPGA flags, including the TX underrun indicator in bit zero.
    pub fn meta_flags(&self) -> u32 {
        self.meta_flags
    }

    /// Recognizes the zero TX prefix or stock FPGA's `0x12344321` RX marker.
    ///
    /// This check applies to SC16 timestamp headers, not packet metadata.
    pub fn is_valid_meta_format(&self) -> bool {
        (self.reserved_or_length == 0 && self.flags_or_core == 0)
            || (self.reserved_or_length == 0x4321 && self.flags_or_core == 0x1234)
    }

    /// Returns raw byte three; this is part of the SC16 diagnostic marker.
    pub fn stream_flags(&self) -> u8 {
        (self.flags_or_core >> 8) as u8
    }

    /// Returns raw byte two; this is not a negotiated metadata version.
    pub fn meta_version(&self) -> u8 {
        self.flags_or_core as u8
    }

    /// Returns the packet payload length in 32-bit words, excluding its header.
    pub fn packet_length(&self) -> u16 {
        self.reserved_or_length
    }

    /// Returns the packet source core ID from byte three.
    pub fn packet_core_id(&self) -> u8 {
        (self.flags_or_core >> 8) as u8
    }

    /// Returns the packet flags from byte two.
    pub fn packet_flags(&self) -> u8 {
        self.flags_or_core as u8
    }
}

/// Validated fixed-message layout for timestamped SC16 streaming.
///
/// Created from the connected device's versions and USB speed. The layout is
/// immutable while a stream owns its endpoint claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MetadataLayout {
    message_size: usize,
}

impl MetadataLayout {
    pub(crate) fn for_versions(
        speed: Speed,
        fpga: SemanticVersion,
        firmware: SemanticVersion,
    ) -> Result<Self> {
        let modern = firmware >= SemanticVersion::new(2, 5, 0);
        if modern != (fpga >= SemanticVersion::new(0, 16, 0)) {
            return Err(Error::Unsupported(
                "firmware and FPGA use different metadata message sizes",
            ));
        }
        let message_size = match (speed, modern) {
            (Speed::High, false) => 1024,
            (Speed::High, true) => 4096,
            (Speed::Super | Speed::SuperPlus, false) => 2048,
            (Speed::Super | Speed::SuperPlus, true) => 8192,
            _ => return Err(Error::UnsupportedSpeed),
        };
        Ok(Self { message_size })
    }

    /// Returns the complete FPGA message size, including its header.
    pub fn message_size(self) -> usize {
        self.message_size
    }

    /// Returns the number of complex SC16 samples following each header.
    pub fn samples_per_message(self) -> usize {
        (self.message_size - METADATA_HEADER_SIZE) / 4
    }

    /// Views every complete message in a USB buffer without copying its samples.
    ///
    /// # Errors
    /// Returns [`Error::MetadataLength`] for a partial message. Empty buffers
    /// yield no messages. Raw header flags are preserved, not interpreted as samples.
    pub fn messages(
        self,
        bytes: &[u8],
    ) -> Result<impl ExactSizeIterator<Item = MetadataMessage<'_>> + DoubleEndedIterator + Clone>
    {
        if !bytes.len().is_multiple_of(self.message_size) {
            return Err(Error::MetadataLength {
                required: bytes.len().next_multiple_of(self.message_size),
                actual: bytes.len(),
            });
        }
        Ok(bytes
            .chunks_exact(self.message_size)
            .map(|message| MetadataMessage {
                header: MetadataHeader::from_bytes(message).unwrap(),
                payload: &message[METADATA_HEADER_SIZE..],
            }))
    }
}

/// A decoded timestamp header and borrowed SC16 payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataMessage<'a> {
    header: MetadataHeader,
    payload: &'a [u8],
}

impl<'a> MetadataMessage<'a> {
    /// Returns the message's decoded header.
    pub fn header(self) -> MetadataHeader {
        self.header
    }

    /// Returns little-endian SC16 samples, excluding the message's header.
    pub fn payload(self) -> &'a [u8] {
        self.payload
    }
}

/// A packet header and its declared 32-bit-word payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataPacket<'a> {
    header: MetadataHeader,
    payload: &'a [u8],
}

impl<'a> MetadataPacket<'a> {
    /// Decodes one packet and returns any trailing transport bytes separately.
    ///
    /// The payload length is in 32-bit words and excludes the 16-byte header.
    /// The remainder may contain transport padding; it is never included in
    /// the payload or silently discarded.
    ///
    /// # Errors
    /// Returns [`Error::MetadataLength`] for a truncated header or payload.
    pub fn parse_prefix(bytes: &'a [u8]) -> Result<(Self, &'a [u8])> {
        let header = MetadataHeader::from_bytes(bytes).ok_or(Error::MetadataLength {
            required: METADATA_HEADER_SIZE,
            actual: bytes.len(),
        })?;
        let end = METADATA_HEADER_SIZE + usize::from(header.packet_length()) * 4;
        let payload = bytes
            .get(METADATA_HEADER_SIZE..end)
            .ok_or(Error::MetadataLength {
                required: end,
                actual: bytes.len(),
            })?;
        Ok((Self { header, payload }, &bytes[end..]))
    }

    /// Returns the packet's decoded header.
    pub fn header(self) -> MetadataHeader {
        self.header
    }

    /// Returns exactly the declared payload bytes, excluding transport padding.
    pub fn payload(self) -> &'a [u8] {
        self.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_tracks_version_boundaries_and_usb_speed() {
        for speed in [Speed::High, Speed::Super, Speed::SuperPlus] {
            let modern = MetadataLayout::for_versions(
                speed,
                SemanticVersion::new(0, 16, 0),
                SemanticVersion::new(2, 5, 0),
            )
            .unwrap();
            let legacy = MetadataLayout::for_versions(
                speed,
                SemanticVersion::new(0, 15, 9),
                SemanticVersion::new(2, 4, 9),
            )
            .unwrap();
            assert_eq!(
                modern.message_size(),
                if speed == Speed::High { 4096 } else { 8192 }
            );
            assert_eq!(
                legacy.message_size(),
                if speed == Speed::High { 1024 } else { 2048 }
            );
            assert!(
                MetadataLayout::for_versions(
                    speed,
                    SemanticVersion::new(0, 15, 9),
                    SemanticVersion::new(2, 5, 0)
                )
                .is_err()
            );
            assert!(
                MetadataLayout::for_versions(
                    speed,
                    SemanticVersion::new(0, 16, 0),
                    SemanticVersion::new(2, 4, 9)
                )
                .is_err()
            );
        }
    }

    #[test]
    fn multiple_messages_preserve_each_header_and_payload_boundary() {
        for size in [1024, 2048, 4096, 8192] {
            let layout = MetadataLayout { message_size: size };
            let mut bytes = vec![0; size * 3];
            for (index, message) in bytes.chunks_exact_mut(size).enumerate() {
                message[..16].copy_from_slice(
                    &MetadataHeader::new(
                        0x4321,
                        0x1234,
                        index as u64 * layout.samples_per_message() as u64,
                        2,
                    )
                    .to_bytes(),
                );
                message[16..].fill(index as u8);
            }
            for (index, message) in layout.messages(&bytes).unwrap().enumerate() {
                assert!(message.header().is_valid_meta_format());
                assert_eq!(
                    message.header().timestamp(),
                    index as u64 * layout.samples_per_message() as u64
                );
                assert_eq!(message.payload().len(), size - 16);
                assert!(message.payload().iter().all(|&byte| byte == index as u8));
            }
            assert!(layout.messages(&bytes[..bytes.len() - 1]).is_err());
            assert_eq!(layout.messages(&[]).unwrap().len(), 0);
        }
    }

    #[test]
    fn packet_length_counts_words_and_trailing_transport_bytes_are_explicit() {
        let mut bytes = MetadataHeader::new(3, 0x12ab, 0x0102_0304_0506_0708, 0)
            .to_bytes()
            .to_vec();
        bytes.extend_from_slice(&[1; 12]);
        bytes.extend_from_slice(&[0; 4]);
        let (packet, padding) = MetadataPacket::parse_prefix(&bytes).unwrap();
        assert_eq!(packet.header().packet_length(), 3);
        assert_eq!(packet.header().packet_core_id(), 0x12);
        assert_eq!(packet.header().packet_flags(), 0xab);
        assert_eq!(packet.payload(), &[1; 12]);
        assert_eq!(padding, &[0; 4]);
        for length in 0..28 {
            assert!(MetadataPacket::parse_prefix(&bytes[..length]).is_err());
        }
    }
}
