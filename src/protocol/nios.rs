//! NIOS packet format and address/data width families.
//!
//! Defines the packet structure used to communicate with the NIOS II
//! soft-core processor. Supports 8x8, 8x16, 8x32, 8x64, and 32x32
//! address/data width combinations. Provides generic encode/decode
//! functions for issuing read and write commands.

pub mod packet_generic;
pub mod targets;
use crate::error::Error;
use crate::protocol::nios::packet_generic::{NiosNum, NiosPktDecoder};
pub use packet_generic::{NiosPacket, NiosPkt, NiosPktFlags, NiosPktStatus};
pub use targets::{
    NiosPkt8x8Target, NiosPkt8x16AddrIqCorr, NiosPkt8x16Target, NiosPkt8x32Target,
    NiosPkt8x64Target, NiosPkt8x64TimestampAddr, NiosPkt32x32Target,
};

/// Error conditions produced during NIOS packet encode/decode operations.
#[derive(thiserror::Error, Debug)]
pub enum NiosPacketError {
    /// The integer PLL divider exceeds its nine-bit wire representation.
    #[error("nint value {0} exceeds maximum 0x1ff")]
    NintOverflow(u16),
    /// The nfrac value exceeds the maximum representable value of 0x7FFFFF.
    #[error("nfrac value {0} exceeds maximum 0x7FFFFF")]
    NfracOverflow(u32),
    /// The freqsel value exceeds the maximum allowed range.
    #[error("freqsel value {0} exceeds maximum {1}")]
    FreqselOverflow(u8, u8),
    /// The vcocap value exceeds the maximum allowed range.
    #[error("vcocap value {0} exceeds maximum {1}")]
    VcocapOverflow(u8, u8),
    /// The packet buffer is not the expected 16 bytes.
    #[error("invalid packet size: expected 16 bytes, got {0}")]
    InvalidSize(usize),
    /// The requested address/data size combination has no defined magic byte.
    #[error("unsupported address/data size combination")]
    InvalidTypeCombination,
    /// The NIOS write command did not return a success status.
    #[error("NIOS write command failed")]
    WriteFailed,
    /// The NIOS read command did not return a success status.
    #[error("NIOS read command failed")]
    ReadFailed,
    /// The response belongs to another packet family.
    #[error("unexpected NIOS magic: expected {expected:#04x}, got {actual:#04x}")]
    MagicMismatch {
        /// Required packet-family magic byte.
        expected: u8,
        /// Received magic byte.
        actual: u8,
    },
    /// The response does not match the requested target, address, or direction.
    #[error("NIOS response does not match the request")]
    ResponseMismatch,
}

/// Encodes a NIOS read request into `buf`.
///
/// Writes a packet targeting `target` at address `addr` with the read
/// flag set. Requires `buf` to be at least 16 bytes.
pub fn nios_encode_read<A: NiosNum, D: NiosNum>(
    buf: &mut [u8],
    target: u8,
    addr: A,
) -> Result<(), Error> {
    NiosPkt::<A, D>::new(buf)?.prepare_read(target, addr);
    Ok(())
}

/// Encodes a NIOS write request into `buf`.
///
/// Writes a packet targeting `target` at address `addr` with the
/// write flag set and `data` payload. Requires `buf` to be at least 16 bytes.
pub fn nios_encode_write<A: NiosNum, D: NiosNum>(
    buf: &mut [u8],
    target: u8,
    addr: A,
    data: D,
) -> Result<(), Error> {
    NiosPkt::<A, D>::new(buf)?.prepare_write(target, addr, data);
    Ok(())
}

/// Decodes the data payload from a NIOS read response.
///
/// Extracts the response data from the packet at the offset determined
/// by the address-type width `A` and data-type width `D`.
pub fn nios_decode_read<A: NiosNum, D: NiosNum>(response: &[u8]) -> Result<D, Error> {
    NiosPkt::<A, D>::validate_response(response, NiosPktFlags::Read)?;
    NiosPktDecoder::decode_data::<A, D>(response)
}

/// Decodes a NIOS write response and verifies success.
///
/// Returns `Ok(())` if the success flag is set, or `WriteFailed` otherwise.
pub fn nios_decode_write<A: NiosNum, D: NiosNum>(response: &[u8]) -> Result<(), Error> {
    NiosPkt::<A, D>::validate_response(response, NiosPktFlags::Write)
}

#[cfg(feature = "bladerf1")]
pub(crate) fn validate_response_address<A: NiosNum>(
    response: &[u8],
    target: u8,
    address: A,
) -> Result<(), Error> {
    if response.len() != 16 {
        return Err(NiosPacketError::InvalidSize(response.len()).into());
    }
    if response[1] != target || response[4..4 + A::SIZE] != *address.to_le_bytes().as_ref() {
        return Err(NiosPacketError::ResponseMismatch.into());
    }
    Ok(())
}

#[cfg(all(test, feature = "bladerf1"))]
mod tests {
    use super::*;

    #[test]
    fn response_identity_includes_target_and_full_address() {
        let mut response = [0; 16];
        nios_encode_read::<u32, u32>(&mut response, 2, 0x1234_5678).unwrap();
        assert!(validate_response_address(&response, 2, 0x1234_5678u32).is_ok());
        assert!(validate_response_address(&response, 3, 0x1234_5678u32).is_err());
        assert!(validate_response_address(&response, 2, 0x1334_5678u32).is_err());
    }
}
