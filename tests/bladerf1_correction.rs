mod common;

use crate::common::*;

use bladerf_globals::bladerf1::BladeRf1Correction;
use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};
use libbladerf_rs::Result;

#[test]
fn gain_correction() -> Result<()> {
    logging_init("bladerf1_correction");

    let correction_type = &BladeRf1Correction::Gain;

    for channel in [BLADERF_MODULE_RX, BLADERF_MODULE_TX] {
        for desired in [-4096, 4096] {
            // TODO: What channels are supported?
            let current = BLADERF.get_correction(channel, correction_type)?;
            log::trace!(
                "Channel {channel} {correction_type:?}-Gain Correction (CURRENT):\t{current}"
            );
            log::trace!(
                "Channel {channel} {correction_type:?}-Gain Correction (DESIRED):\t{desired}"
            );

            BLADERF.set_correction(channel, correction_type, desired)?;

            let new = BLADERF.get_correction(channel, correction_type)?;
            log::trace!("Channel {channel} {correction_type:?}-Gain Correction (NEW):\t{new}");
            assert_eq!(new, desired);
        }
    }

    // BLADERF.device_reset()
    Ok(())
}

#[test]
fn phase_correction() -> Result<()> {
    logging_init("bladerf1_correction");

    let correction_type = &BladeRf1Correction::Phase;

    for channel in [BLADERF_MODULE_RX, BLADERF_MODULE_TX] {
        for desired in [-4096, 4096] {
            // TODO: What channels are supported?
            let current = BLADERF.get_correction(channel, correction_type)?;
            log::trace!(
                "Channel {channel} {correction_type:?}-Gain Correction (CURRENT):\t{current}"
            );
            log::trace!(
                "Channel {channel} {correction_type:?}-Gain Correction (DESIRED):\t{desired}"
            );

            BLADERF.set_correction(channel, correction_type, desired)?;

            let new = BLADERF.get_correction(channel, correction_type)?;
            log::trace!("Channel {channel} {correction_type:?}-Gain Correction (NEW):\t{new}");
            assert_eq!(new, desired);
        }
    }

    // BLADERF.device_reset()
    Ok(())
}

#[test]
fn iq_correction() -> Result<()> {
    logging_init("bladerf1_correction");

    for channel in [BLADERF_MODULE_RX, BLADERF_MODULE_TX] {
        for correction_type in &[BladeRf1Correction::DcoffI, BladeRf1Correction::DcoffQ] {
            for desired in [-2048, 2048] {
                // TODO: Check, why the correction only goes until +-2016 instead of +-2048
                // for desired in [-2016, 2016] {
                // TODO: What channels are supported?
                let current = BLADERF.get_correction(channel, correction_type)?;
                log::trace!(
                    "Channel {channel} {correction_type:?}-Gain Correction (CURRENT):\t{current}"
                );
                log::trace!(
                    "Channel {channel} {correction_type:?}-Gain Correction (DESIRED):\t{desired}"
                );

                BLADERF.set_correction(channel, correction_type, desired)?;

                let new = BLADERF.get_correction(channel, correction_type)?;
                log::trace!("Channel {channel} {correction_type:?}-Gain Correction (NEW):\t{new}");
                assert_eq!(new, desired);
            }
        }
    }

    // BLADERF.device_reset()
    Ok(())
}
