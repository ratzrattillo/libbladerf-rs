use crate::{BladeRf1, BladeRfError};
use anyhow::Result;
use bladerf_globals::bladerf1::{
    BLADERF_GPIO_PACKET, BLADERF_GPIO_TIMESTAMP, BLADERF_GPIO_TIMESTAMP_DIV2,
};
use bladerf_globals::{BladeRfDirection, BladerfFormat};
use num_complex::Complex32;
use nusb::transfer::{Bulk, ControlIn, ControlType, In, Recipient};
use std::io::BufRead;
use std::time::Duration;

impl BladeRf1 {
    /// Perform the neccessary device configuration for the specified format
    /// (e.g., enabling/disabling timestamp support), first checking that the
    /// requested format would not conflict with the other stream direction.
    ///
    ///      dev     Device handle
    ///      dir     Direction that is currently being configured
    ///      format  Format the channel is being configured for
    ///
    /// @return 0 on success, BLADERF_ERR_* on failure
    pub async fn perform_format_config(
        &self,
        dir: BladeRfDirection,
        format: BladerfFormat,
    ) -> Result<()> {
        // BladerfFormatPacketMeta
        //struct bladerf1_board_data *board_data = dev->board_data;

        //int status = 0;
        let mut use_timestamps: bool = false;
        let _other_using_timestamps: bool = false;

        // status = requires_timestamps(format, &use_timestamps);
        // if (status != 0) {
        //     log_debug("%s: Invalid format: %d\n", __FUNCTION__, format);
        //     return status;
        // }

        let _other = match dir {
            BladeRfDirection::Rx => BladeRfDirection::Tx,
            BladeRfDirection::Tx => BladeRfDirection::Rx,
        };

        // status = requires_timestamps(board_data->module_format[other],
        //     &other_using_timestamps);

        // if ((status == 0) && (other_using_timestamps != use_timestamps)) {
        //     log_debug("Format conflict detected: RX=%d, TX=%d\n");
        //     return BLADERF_ERR_INVAL;
        // }

        let mut gpio_val = self.config_gpio_read().await?;

        log::debug!("gpio_val {gpio_val:#08x}");
        if format == BladerfFormat::PacketMeta {
            gpio_val |= BLADERF_GPIO_PACKET;
            use_timestamps = true;
            log::debug!("BladerfFormat::PacketMeta");
        } else {
            gpio_val &= !BLADERF_GPIO_PACKET;
            log::debug!("else");
        }
        log::debug!("gpio_val {gpio_val:#08x}");

        if use_timestamps {
            log::debug!("use_timestamps");
            gpio_val |= BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2;
        } else {
            log::debug!("dont use_timestamps");
            gpio_val &= !(BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2);
        }

        log::debug!("gpio_val {gpio_val:#08x}");

        self.config_gpio_write(gpio_val).await?;
        // if (status == 0) {
        //     board_data->module_format[dir] = format;
        // }

        //return status;
        Ok(())
    }

    /**
     * Deconfigure and update any state pertaining what a format that a stream
     * direction is no longer using.
     *
     *    dev     Device handle
     *    dir     Direction that is currently being deconfigured
     *
     * @return 0 on success, BLADERF_ERR_* on failure
     */
    pub fn perform_format_deconfig(&self, direction: &BladeRfDirection) -> Result<()> {
        //struct bladerf1_board_data *board_data = dev->board_data;

        match direction {
            BladeRfDirection::Rx | BladeRfDirection::Tx => {
                /* We'll reconfigure the HW when we call perform_format_config, so
                 * we just need to update our stored information */
                //board_data -> module_format[dir] = - 1;
            }
        }

        Ok(())
    }

    pub async fn experimental_control_urb(&self) -> Result<()> {
        // TODO: Dont know what this is doing
        let pkt = ControlIn {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: 0x4,
            value: 0x1,
            index: 0,
            length: 0x4,
        };
        let vec = self
            .interface
            .control_in(pkt, Duration::from_secs(5))
            .await?;
        log::debug!("Control Response Data: {vec:?}");
        Ok(())
    }

    pub fn read_sync(&self, buffers: &mut [&mut [Complex32]], timeout_us: i64) -> Result<usize> {
        let num_channels = buffers.len();
        log::debug!("num_channels: {num_channels}");
        // TODO: What happens if buffers for the different channels have different sizes? Is that possible?
        let buffer_size = buffers[0].len();
        log::debug!("buffer_size: {buffer_size}");

        if buffers.is_empty() || buffers[0].is_empty() {
            log::debug!("no buffers available, or buffers have a length of zero!");
            return Ok(0);
        }
        if buffers.len() > 1 {
            log::debug!(
                "bladerf1 only supports reading from one RX channel. Please provide a one dimensional buffer!"
            );
            return Err(BladeRfError::Unsupported.into());
        }

        let ep_bulk_in = self.interface.endpoint::<Bulk, In>(0x81)?;
        let mut reader = ep_bulk_in
            .reader(buffer_size)
            .with_num_transfers(1)
            .with_read_timeout(Duration::from_micros(timeout_us as u64));

        let buf = reader.fill_buf()?;

        for (idx, sample) in buf
            .chunks_exact(4)
            .map(|buf| buf.split_at(2))
            .map(|(re, im)| {
                (
                    i16::from_le_bytes(<[u8; 2]>::try_from(re).unwrap()) as f32,
                    i16::from_le_bytes(<[u8; 2]>::try_from(im).unwrap()) as f32,
                )
            })
            .map(|(re, im)| Complex32::new(re, im))
            .enumerate()
        {
            // log::debug!("{sample}");
            buffers[0][idx] = sample;
        }

        // mark the bytes we worked with as read
        let length = buf.len();
        log::debug!("buffer length: {length}");
        reader.consume(length);
        log::debug!("consumed length: {length}");

        Ok(length / 4)
    }

    // pub fn write_all_sync(
    //     &mut self,
    //     buffers: &[&[Complex32]],
    //     _at_ns: Option<i64>,
    //     _end_burst: bool,
    //     timeout_us: i64,
    // ) -> Result<()> {
    //     let num_buffers = buffers.len();
    //     // This length may not be set in stone for every buffer in buffers.
    //     let buffer_size = buffers[0].len();
    //
    //     if buffers.is_empty() || buffers[0].is_empty() {
    //         return Ok(());
    //     }
    //
    //     let ep_bulk_out = self.interface.endpoint::<Bulk, Out>(0x01)?;
    //     let writer = ep_bulk_out.writer(buffer_size).with_num_transfers(num_buffers).with_write_timeout(Duration::from_micros(timeout_us as u64));
    //     for buffer in buffers {
    //
    //     }
    //
    //     Ok(())
    // }

    pub async fn async_run_stream(&self) -> Result<()> {
        // TODO: In_ENDPOINT is 0x81 here, not 0x82
        let mut ep_bulk_in = self.interface.endpoint::<Bulk, In>(0x81)?;

        let n_transfers = 8;
        let factor = 32;
        // let factor = match self.device.speed().unwrap_or(Speed::Low) {
        //     // TODO: These numbers are completely made up.
        //     // TODO: They should be based on real USB Frame sizes depending on the given Speed
        //     Speed::Low => 8,
        //     Speed::Full => 16,
        //     Speed::High => 32,
        //     Speed::Super => 32, // This factor is used by the original libusb libbladerf implementation.
        //     Speed::SuperPlus => 96,
        //     _ => 8,
        // };

        let max_packet_size = ep_bulk_in.max_packet_size();
        let max_frame_size = max_packet_size * factor;
        log::debug!("Max Packet Size: {max_packet_size}");

        for _i in 0..n_transfers {
            let buffer = ep_bulk_in.allocate(max_frame_size);
            ep_bulk_in.submit(buffer);
            // log::debug!("submitted_transfers: {i}");
        }

        loop {
            let result = ep_bulk_in.next_complete().await;
            log::debug!("{result:?}");
            if result.status.is_err() {
                break;
            }
            ep_bulk_in.submit(result.buffer);
        }
        Ok(())
    }

    // pub async fn bladerf1_stream(&self, stream: &bladerf_stream, layout: BladeRfChannelLayout) -> Result<()> {
    //     let dir: BladeRfDirection = layout & BLADERF_DIRECTION_MASK;
    //     let stream_status: i32;
    //
    //     // if layout != BladeRfChannelLayout::BladerfRxX1 && layout != BladeRfChannelLayout::BladerfTxX1 {
    //     //     return Err(anyhow!("Invalid ChannelLayout"));
    //     // }
    //
    //     self.perform_format_config(dir, stream->format)?;
    //
    //     stream_status = self.async_run_stream(stream, layout).await;
    //     // TODO: static void LIBUSB_CALL lusb_stream_cb
    //
    //     self.perform_format_deconfig(dir)?;
    // }
}
