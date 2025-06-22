use crate::BladeRf1;
use crate::hardware::lms6002d::{BLADERF1_BAND_HIGH, LMS6002D};
use anyhow::Result;
use anyhow::anyhow;
use bladerf_globals::SdrRange;
use bladerf_globals::bladerf1::{BLADERF_FREQUENCY_MAX, BLADERF_FREQUENCY_MIN};
use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest};

enum TuningMode {
    Host,
    Fpga,
}

impl BladeRf1 {
    pub async fn set_frequency(&self, channel: u8, frequency: u64) -> Result<()> {
        //let dc_cal = if channel == bladerf_channel_rx!(0) { cal_dc.rx } else { cal.dc_tx };

        println!("Setting Frequency on channel {channel} to {frequency}Hz");

        // Ommit XB200 settings here

        // TODO: The tuning mode should be read from the board config
        // In the packet captures, this is where the changes happen:
        // -  Packet No. 317 in rx-BladeRFTest-unix-filtered.pcapng
        // -  Packet No. 230 in rx-rusttool-filtered.pcapng
        // This is maybe due to the tuning mode being FPGA and not Host
        let mode = TuningMode::Fpga;

        // For tuning HOST Tuning Mode:
        match mode {
            TuningMode::Host => {
                self.lms.set_frequency(channel, frequency as u32).await?;
                let band = if frequency < BLADERF1_BAND_HIGH as u64 {
                    Band::Low
                } else {
                    Band::High
                };
                self.band_select(channel, band).await?;
            }
            TuningMode::Fpga => {
                self.lms
                    .schedule_retune(
                        channel,
                        NiosPktRetuneRequest::RETUNE_NOW,
                        frequency as u32,
                        None,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn get_frequency(&self, channel: u8) -> Result<u32> {
        let f = self.lms.get_frequency(channel).await?;
        if f.x == 0 {
            /* If we see this, it's most often an indication that communication
             * with the LMS6002D is not occuring correctly */
            return Err(anyhow!("LMSFreq.x was zero!"));
        }
        let frequency_hz = LMS6002D::frequency_to_hz(&f);

        // if (dev->xb == BLADERF_XB_200) {
        //     status = xb200_get_path(dev, ch, &path);
        //     if (status != 0) {
        //         return status;
        //     }
        //     if (path == BLADERF_XB200_MIX) {
        //         *frequency = 1248000000 - *frequency;
        //     }
        // }

        Ok(frequency_hz)
    }

    pub fn get_frequency_range() -> SdrRange<u32> {
        SdrRange {
            min: BLADERF_FREQUENCY_MIN,
            max: BLADERF_FREQUENCY_MAX,
            step: 1,
            scale: 1,
        }
        // if (dev->xb == BLADERF_XB_200) {
        //     SdrRange {
        //             min: 0,
        //             max: BLADERF_FREQUENCY_MAX,
        //             step: 1,
        //             scale: 1,
        //         }
        // } else {
        //     SdrRange {
        //             min: BLADERF_FREQUENCY_MIN,
        //             max: BLADERF_FREQUENCY_MAX,
        //             step: 1,
        //             scale: 1,
        //         }
        // }
    }

    pub async fn select_band(&self, channel: u8, frequency: u32) -> Result<u32> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let band = if frequency < BLADERF1_BAND_HIGH {
            Band::Low
        } else {
            Band::High
        };

        self.band_select(channel, band).await
    }
}
