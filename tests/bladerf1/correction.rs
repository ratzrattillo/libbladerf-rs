use super::common::*;
use libbladerf_rs::bladerf1::board::Correction;
use libbladerf_rs::{Channel, Result};

fn roundtrip_correction(type_name: &Correction, values: [i16; 2]) -> Result<()> {
    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;

    for channel in [Channel::Rx, Channel::Tx] {
        let current = rf.get_correction(channel, type_name)?;

        for &desired in &values {
            log::trace!("Channel {channel:?} {type_name:?} Correction (CURRENT):\t{current}");
            log::trace!("Channel {channel:?} {type_name:?} Correction (DESIRED):\t{desired}");

            rf.set_correction(channel, type_name, desired)?;

            let new = rf.get_correction(channel, type_name)?;
            log::trace!("Channel {channel:?} {type_name:?} Correction (NEW):\t{new}");
            assert_eq!(new, desired);
        }

        rf.set_correction(channel, type_name, current)?;
    }

    Ok(())
}

#[test]
fn gain_correction() -> Result<()> {
    logging_init("bladerf1_correction");
    roundtrip_correction(&Correction::Gain, [-4096, 4_096])
}

#[test]
fn phase_correction() -> Result<()> {
    logging_init("bladerf1_correction");
    roundtrip_correction(&Correction::Phase, [-4096, 4_096])
}

#[test]
fn iq_correction() -> Result<()> {
    logging_init("bladerf1_correction");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;

    for channel in [Channel::Rx, Channel::Tx] {
        let desired_arr = match channel {
            Channel::Rx => [-2016, -1984, -1952, 0, 1_952, 1_984, 2_016],
            Channel::Tx => [-2032, -2016, -2000, 0, 2_000, 2_016, 2_032],
        };

        for correction_type in &[Correction::DcOffI, Correction::DcOffQ] {
            let current = rf.get_correction(channel, correction_type)?;

            for desired in desired_arr {
                log::trace!(
                    "Channel {channel:?} {correction_type:?} Correction (CURRENT):\t{current}"
                );
                log::trace!(
                    "Channel {channel:?} {correction_type:?} Correction (DESIRED):\t{desired}"
                );

                rf.set_correction(channel, correction_type, desired)?;

                let new = rf.get_correction(channel, correction_type)?;
                log::trace!("Channel {channel:?} {correction_type:?} Correction (NEW):\t{new}");

                assert_eq!(new, desired);
            }

            rf.set_correction(channel, correction_type, current)?;
        }
    }

    Ok(())
}
