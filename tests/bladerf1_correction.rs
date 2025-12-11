mod common;

use crate::common::*;

use libbladerf_rs::bladerf1::Correction;
use libbladerf_rs::{Channel, Result};

#[test]
fn gain_correction() -> Result<()> {
    logging_init("bladerf1_correction");

    let correction_type = &Correction::Gain;

    for channel in [Channel::Rx, Channel::Tx] {
        for desired in [-4096, 4096] {
            // TODO: What channels are supported?
            let current = BLADERF.get_correction(channel, correction_type)?;
            log::trace!("Channel {channel:?} {correction_type:?} Correction (CURRENT):\t{current}");
            log::trace!("Channel {channel:?} {correction_type:?} Correction (DESIRED):\t{desired}");

            BLADERF.set_correction(channel, correction_type, desired)?;

            let new = BLADERF.get_correction(channel, correction_type)?;
            log::trace!("Channel {channel:?} {correction_type:?} Correction (NEW):\t{new}");
            assert_eq!(new, desired);
        }
    }

    // BLADERF.device_reset()
    Ok(())
}

#[test]
fn phase_correction() -> Result<()> {
    logging_init("bladerf1_correction");

    let correction_type = &Correction::Phase;

    for channel in [Channel::Rx, Channel::Tx] {
        for desired in [-4096, 4096] {
            // TODO: What channels are supported?
            let current = BLADERF.get_correction(channel, correction_type)?;
            log::trace!("Channel {channel:?} {correction_type:?} Correction (CURRENT):\t{current}");
            log::trace!("Channel {channel:?} {correction_type:?} Correction (DESIRED):\t{desired}");

            BLADERF.set_correction(channel, correction_type, desired)?;

            let new = BLADERF.get_correction(channel, correction_type)?;
            log::trace!("Channel {channel:?} {correction_type:?} Correction (NEW):\t{new}");
            assert_eq!(new, desired);
        }
    }

    // BLADERF.device_reset()
    Ok(())
}

#[test]
fn iq_correction() -> Result<()> {
    logging_init("bladerf1_correction");

    for channel in [Channel::Rx, Channel::Tx] {
        // Arbitrary desired values are not possible, as they are shrinked to fit into only 8/7 bit register
        // TODO: Check why this scaling is even required/desired. Maybe for compatibility of the API with BladeRf2 ???
        let desired_arr = match channel {
            Channel::Rx => {
                // RX I/Q offset correction is specified in registers 0x71 and 0x72 of the LMS.
                // DCOFF_<I/Q>_RXFE[6:0]: DC offset cancellation, I/Q channel (7 Bit).
                // Code format: Sign(<6>)-Magnitude(<5:0>), signed magnitude format
                // Input to the offset correction methods expects values in the range of -2048 until 2048.
                // When setting an offset correction, these values are scaled down to fit into the limited 7bit register space.
                // When getting an offset correction, these values are scaled up again to the -2048 until 2048 range.
                // This up/down scaling is lossy. Only values can be restored, that are representable in the 7bit register space.
                // Valid steps are 2⁵ (32) (E.g. from -2048 till -1985 every value will be scalded down to 0xff and upscaled to -2016)
                [-2016, -1984, -1952, 0, 1952, 1984, 2016]
            }
            Channel::Tx => {
                // TX I/Q offset correction is specified in registers 0x42 and 0x43 of the LMS.
                // VGA1DC_<I/Q>[7:0]: TXVGA1 DC shift control, LO leakage cancellation (8 Bit)
                // This register is special, as the MSB is an inverted sign bit (1 = positive, 0 = negative)
                // LSB=0.0625mV, encoded as shown below:
                //     code DC Shift [mV]
                //     =======================
                //     00000000 -16
                //     …
                //     01111111 -0.0625
                //     10000000 0 (default)
                //     10000001 0.0625
                //     …
                //     11111111 15.9375
                // Input to the offset correction methods expects values in the range of -2048 until 2048.
                // When setting an offset correction, these values are scaled down to fit into the limited 8bit register space.
                // When getting an offset correction, these values are scaled up again to the -2048 until 2048 range.
                // This up/down scaling is lossy. Only values can be restored, that are representable in the 8bit register space.
                // Valid steps are 2⁴ (16)
                [-2032, -2016, -2000, 0, 2000, 2016, 2032]
            }
        };

        for correction_type in &[Correction::DcoffI, Correction::DcoffQ] {
            for desired in desired_arr {
                // TODO: What channels are supported?
                let current = BLADERF.get_correction(channel, correction_type)?;
                log::trace!(
                    "Channel {channel:?} {correction_type:?} Correction (CURRENT):\t{current}"
                );
                log::trace!(
                    "Channel {channel:?} {correction_type:?} Correction (DESIRED):\t{desired}"
                );

                BLADERF.set_correction(channel, correction_type, desired)?;

                let new = BLADERF.get_correction(channel, correction_type)?;
                log::trace!("Channel {channel:?} {correction_type:?} Correction (NEW):\t{new}");

                assert_eq!(new, desired);
            }
        }
    }

    // BLADERF.device_reset()
    Ok(())
}
