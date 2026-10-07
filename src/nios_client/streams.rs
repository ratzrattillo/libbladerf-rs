use crate::bladerf1::{GpioFlags, SampleFormat};
use crate::{Channel, Error, Result};
use std::sync::{Arc, Weak};

const FORMAT_MASK: GpioFlags = GpioFlags::PACKET
    .union(GpioFlags::TIMESTAMP)
    .union(GpioFlags::TIMESTAMP_DIV2)
    .union(GpioFlags::EIGHT_BIT_MODE)
    .union(GpioFlags::HIGHLY_PACKED_MODE);
const STREAM_OWNED_GPIO_MASK: GpioFlags = FORMAT_MASK.union(GpioFlags::from_bits_retain(0x07));

impl GpioFlags {
    pub(crate) fn set_stream_format(&mut self, format: Option<StreamFormat>) {
        self.remove(FORMAT_MASK);
        self.insert(format.map_or(Self::empty(), StreamFormat::gpio_flags));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamFormat {
    Samples,
    Timestamps,
    Packets,
}

impl TryFrom<SampleFormat> for StreamFormat {
    type Error = Error;

    fn try_from(format: SampleFormat) -> Result<Self> {
        match format {
            SampleFormat::Sc16Q11 => Ok(Self::Samples),
            SampleFormat::Sc16Q11Meta => Ok(Self::Timestamps),
            SampleFormat::PacketMeta => Ok(Self::Packets),
            _ => Err(Error::Unsupported(
                "sample format is not implemented by the BladeRF1 FPGA",
            )),
        }
    }
}

impl StreamFormat {
    pub(crate) fn supports_versions(
        self,
        fpga: crate::SemanticVersion,
        firmware: crate::SemanticVersion,
    ) -> bool {
        match self {
            Self::Samples => true,
            Self::Timestamps => fpga >= crate::SemanticVersion::new(0, 1, 0),
            Self::Packets => {
                fpga >= crate::SemanticVersion::new(0, 12, 0)
                    && firmware >= crate::SemanticVersion::new(2, 4, 0)
            }
        }
    }
    fn gpio_flags(self) -> GpioFlags {
        match self {
            Self::Samples => GpioFlags::empty(),
            Self::Timestamps => GpioFlags::TIMESTAMP | GpioFlags::TIMESTAMP_DIV2,
            Self::Packets => GpioFlags::TIMESTAMP | GpioFlags::TIMESTAMP_DIV2 | GpioFlags::PACKET,
        }
    }
}

#[derive(Debug)]
pub(crate) struct StreamLease {
    device: Arc<()>,
    owner: Arc<()>,
    channel: Channel,
}

struct Registration {
    owner: Weak<()>,
    format: Option<StreamFormat>,
}

pub(crate) struct StreamClaims {
    device: Arc<()>,
    registrations: [Option<Registration>; 2],
}

impl std::fmt::Debug for StreamClaims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = |channel: Channel| {
            self.registrations[channel as usize]
                .as_ref()
                .map(|registration| (registration.owner.strong_count() != 0, registration.format))
        };
        f.debug_struct("StreamClaims")
            .field("rx_live_and_format", &state(Channel::Rx))
            .field("tx_live_and_format", &state(Channel::Tx))
            .finish()
    }
}

impl Default for StreamClaims {
    fn default() -> Self {
        Self {
            device: Arc::new(()),
            registrations: [None, None],
        }
    }
}

impl StreamClaims {
    fn refresh(&mut self) -> Result<()> {
        for registration in &mut self.registrations {
            if let Some(value) = registration
                && value.owner.strong_count() == 0
            {
                if value.format.is_some() {
                    return Err(Error::RecoveryRequired);
                }
                *registration = None;
            }
        }
        Ok(())
    }

    pub(crate) fn require_idle(&mut self) -> Result<()> {
        self.refresh()?;
        if self.registrations.iter().any(Option::is_some) {
            Err(Error::StreamsActive)
        } else {
            Ok(())
        }
    }

    pub(crate) fn require_no_live_streams(&self) -> Result<()> {
        if self
            .registrations
            .iter()
            .flatten()
            .any(|registration| registration.owner.strong_count() != 0)
        {
            Err(Error::StreamsActive)
        } else {
            Ok(())
        }
    }

    pub(crate) fn require_unclaimed(&mut self, channel: Channel) -> Result<()> {
        self.refresh()?;
        if self.registrations[channel as usize].is_some() {
            Err(Error::StreamClaimed(channel))
        } else {
            Ok(())
        }
    }

    pub(crate) fn claim(&mut self, channel: Channel) -> Result<StreamLease> {
        self.require_unclaimed(channel)?;
        let owner = Arc::new(());
        self.registrations[channel as usize] = Some(Registration {
            owner: Arc::downgrade(&owner),
            format: None,
        });
        Ok(StreamLease {
            device: self.device.clone(),
            owner,
            channel,
        })
    }

    pub(crate) fn check(&self, lease: &StreamLease) -> Result<()> {
        if !Arc::ptr_eq(&self.device, &lease.device) {
            return Err(Error::WrongDevice);
        }
        if self.registrations[lease.channel as usize]
            .as_ref()
            .is_none_or(|registration| !registration.owner.ptr_eq(&Arc::downgrade(&lease.owner)))
        {
            return Err(Error::StreamClosed);
        }
        Ok(())
    }

    pub(crate) fn reserve_format(
        &mut self,
        lease: &StreamLease,
        format: StreamFormat,
    ) -> Result<()> {
        self.check(lease)?;
        self.refresh()?;
        if self
            .registrations
            .iter()
            .flatten()
            .filter_map(|registration| registration.format)
            .any(|active| active != format)
        {
            return Err(Error::IncompatibleStreamFormat);
        }
        self.registrations[lease.channel as usize]
            .as_mut()
            .unwrap()
            .format = Some(format);
        Ok(())
    }

    pub(crate) fn last_format_user(&self, lease: &StreamLease) -> bool {
        self.registrations
            .iter()
            .enumerate()
            .all(|(index, registration)| {
                index == lease.channel as usize
                    || registration
                        .as_ref()
                        .is_none_or(|registration| registration.format.is_none())
            })
    }

    pub(crate) fn release_format(&mut self, lease: &StreamLease) {
        self.registrations[lease.channel as usize]
            .as_mut()
            .unwrap()
            .format = None;
    }

    pub(crate) fn release(&mut self, lease: &StreamLease) {
        self.registrations[lease.channel as usize] = None;
    }

    pub(crate) fn validate_gpio(&mut self, old: GpioFlags, new: GpioFlags) -> Result<()> {
        self.refresh()?;
        if self.registrations.iter().any(Option::is_some)
            && (old ^ new).intersects(STREAM_OWNED_GPIO_MASK)
        {
            return Err(Error::StreamsActive);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn format_users(&self) -> usize {
        self.registrations
            .iter()
            .flatten()
            .filter(|registration| registration.format.is_some())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_updates_replace_only_the_five_format_bits() {
        for raw in [0, 0x57, 0x8004_07d7, u32::MAX]
            .into_iter()
            .chain((0..32).flat_map(|bit| [1 << bit, !(1 << bit)]))
        {
            let mut gpio = GpioFlags::from_bits_retain(raw);
            for (format, expected) in [
                (Some(StreamFormat::Timestamps), 0x0003_0000),
                (Some(StreamFormat::Packets), 0x000b_0000),
                (Some(StreamFormat::Samples), 0),
                (None, 0),
            ] {
                gpio.set_stream_format(format);
                assert_eq!(gpio.bits(), (raw & !0x003b_0000) | expected);
                gpio.set_stream_format(format);
                assert_eq!(gpio.bits(), (raw & !0x003b_0000) | expected);
            }
        }
    }

    #[test]
    fn every_claim_protects_each_owned_gpio_bit_individually() {
        for channel in [Channel::Rx, Channel::Tx] {
            let mut claims = StreamClaims::default();
            let lease = claims.claim(channel).unwrap();
            for format in [
                None,
                Some(StreamFormat::Samples),
                None,
                Some(StreamFormat::Timestamps),
                None,
                Some(StreamFormat::Packets),
                None,
            ] {
                if let Some(format) = format {
                    claims.reserve_format(&lease, format).unwrap();
                } else {
                    claims.release_format(&lease);
                }
                for raw in [0, 0x57, 0x8004_07d7, u32::MAX] {
                    let old = GpioFlags::from_bits_retain(raw);
                    assert!(claims.validate_gpio(old, old).is_ok());
                    for bit in 0..32 {
                        let new = GpioFlags::from_bits_retain(raw ^ (1 << bit));
                        let result = claims.validate_gpio(old, new);
                        if matches!(bit, 0..=2 | 16 | 17 | 19..=21) {
                            assert!(matches!(result, Err(Error::StreamsActive)));
                        } else {
                            result.unwrap();
                        }
                    }
                }
            }
            claims.release(&lease);
            claims
                .validate_gpio(GpioFlags::empty(), GpioFlags::all())
                .unwrap();
        }
    }

    #[test]
    fn gpio_validation_reports_abandoned_active_claims_before_bit_checks() {
        let mut claims = StreamClaims::default();
        let lease = claims.claim(Channel::Rx).unwrap();
        claims
            .reserve_format(&lease, StreamFormat::Samples)
            .unwrap();
        drop(lease);
        for new in [GpioFlags::empty(), GpioFlags::AGC_ENABLE, GpioFlags::all()] {
            assert!(matches!(
                claims.validate_gpio(GpioFlags::empty(), new),
                Err(Error::RecoveryRequired)
            ));
        }
    }

    #[test]
    fn prepared_and_stopped_claims_block_reconfiguration_and_duplicates() {
        let mut claims = StreamClaims::default();
        let lease = claims.claim(Channel::Rx).unwrap();
        assert!(matches!(
            claims.claim(Channel::Rx),
            Err(Error::StreamClaimed(Channel::Rx))
        ));
        assert!(matches!(claims.require_idle(), Err(Error::StreamsActive)));
        assert!(matches!(
            claims.require_no_live_streams(),
            Err(Error::StreamsActive)
        ));
        claims
            .reserve_format(&lease, StreamFormat::Timestamps)
            .unwrap();
        claims.release_format(&lease);
        assert!(matches!(claims.require_idle(), Err(Error::StreamsActive)));
        claims.release(&lease);
        assert!(claims.require_idle().is_ok());
    }

    #[test]
    fn abandoned_running_claim_requires_recovery_but_prepared_claim_is_reclaimable() {
        let mut claims = StreamClaims::default();
        drop(claims.claim(Channel::Rx).unwrap());
        assert!(claims.require_idle().is_ok());
        let lease = claims.claim(Channel::Rx).unwrap();
        claims
            .reserve_format(&lease, StreamFormat::Samples)
            .unwrap();
        drop(lease);
        assert!(matches!(
            claims.require_idle(),
            Err(Error::RecoveryRequired)
        ));
        assert!(claims.require_no_live_streams().is_ok());
    }

    #[test]
    fn capabilities_use_both_version_boundaries_and_stock_fpga_formats() {
        let fpga = crate::SemanticVersion::new(0, 12, 0);
        let firmware = "2.4.0-git-local".parse().unwrap();
        assert!(StreamFormat::Packets.supports_versions(fpga, firmware));
        assert!(
            !StreamFormat::Packets
                .supports_versions(crate::SemanticVersion::new(0, 11, 9), firmware)
        );
        assert!(
            !StreamFormat::Packets.supports_versions(fpga, crate::SemanticVersion::new(2, 3, 9))
        );
        for format in [
            SampleFormat::Sc8Q7,
            SampleFormat::Sc8Q7Meta,
            SampleFormat::Sc16Q11Packed,
        ] {
            assert!(StreamFormat::try_from(format).is_err());
        }
    }
}
