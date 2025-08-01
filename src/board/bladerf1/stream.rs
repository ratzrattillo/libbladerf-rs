use crate::Result;
use crate::{BladeRf1, BladeRf1RxStreamer, BladeRf1TxStreamer, Error};
use bladerf_globals::bladerf1::{
    BLADERF_GPIO_PACKET, BLADERF_GPIO_TIMESTAMP, BLADERF_GPIO_TIMESTAMP_DIV2,
};
use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX, BladeRf1Direction, BladeRf1Format};
use num_complex::Complex32;
use nusb::MaybeFuture;
use nusb::transfer::{Bulk, ControlIn, ControlType, In, Out, Recipient};
use std::io::{BufRead, Write};
use std::thread::sleep;
use std::time::Duration;

impl BladeRf1RxStreamer {
    pub fn new(
        dev: BladeRf1,
        buffer_size: usize,
        num_transfers: Option<usize>,
        timeout: Option<Duration>,
    ) -> Result<Self> {
        let endpoint = dev
            .interface
            .endpoint::<Bulk, In>(0x81)
            .map_err(|e| Error::Nusb(e))?;
        log::trace!(
            "using endpoint 0x81 with buffer_size: {buffer_size}, num_transfers: {num_transfers:?}, timeout: {timeout:?}"
        );
        let mut reader = endpoint.reader(buffer_size);
        if let Some(t) = timeout {
            reader.set_read_timeout(t)
        }
        if let Some(n) = num_transfers {
            reader.set_num_transfers(n)
        }
        Ok(Self {
            dev,
            reader,
            buffer_size,
        })
    }

    pub fn mtu(&self) -> Result<usize> {
        Ok(self.buffer_size)
    }

    pub fn activate(&mut self) -> Result<()> {
        self.dev
            .perform_format_config(BladeRf1Direction::Rx, BladeRf1Format::Sc16Q11)?;
        self.dev.enable_module(BLADERF_MODULE_RX, true)?;
        self.dev.experimental_control_urb()
    }

    pub fn deactivate(&mut self) -> Result<()> {
        self.dev.perform_format_deconfig(BladeRf1Direction::Rx)?;
        self.dev.enable_module(BLADERF_MODULE_RX, false)
    }

    pub fn read_sync(
        &mut self,
        buffers: &mut [&mut [Complex32]],
        timeout_us: i64,
    ) -> Result<usize> {
        let num_channels = buffers.len();

        if buffers.is_empty() || buffers[0].is_empty() {
            log::debug!("no buffers available, or buffers have a length of zero!");
            return Ok(0);
        }
        if num_channels > 1 {
            log::error!(
                "bladerf1 only supports reading from one RX channel. Please provide a one dimensional buffer!"
            );
            return Err(Error::Invalid);
        }

        self.reader
            .set_read_timeout(Duration::from_micros(timeout_us as u64));

        let buf = self.reader.fill_buf()?;

        let mut received = 0;
        for (dst, src) in buffers[0].iter_mut().zip(
            buf.chunks_exact(2 * size_of::<i16>())
                .map(|buf| buf.split_at(2))
                .map(|(re, im)| {
                    (
                        // i16::from_le_bytes(<[u8; 2]>::try_from(re).unwrap()) as f32 / 2047.5,
                        // i16::from_le_bytes(<[u8; 2]>::try_from(im).unwrap()) as f32 / 2047.5,
                        i16::from_le_bytes(<[u8; 2]>::try_from(re).unwrap()) as f32,
                        i16::from_le_bytes(<[u8; 2]>::try_from(im).unwrap()) as f32,
                    )
                })
                .map(|(re, im)| Complex32::new(re, im)),
        ) {
            *dst = src;
            log::trace!("{src}");
            received += 2 * size_of::<i16>();
        }

        self.reader.consume(received);
        log::trace!("consumed length: {received}");

        Ok(received / (2 * size_of::<i16>()))
    }
}

impl BladeRf1TxStreamer {
    pub fn new(
        dev: BladeRf1,
        buffer_size: usize,
        num_transfers: Option<usize>,
        timeout: Option<Duration>,
    ) -> Result<Self> {
        let endpoint = dev.interface.endpoint::<Bulk, Out>(0x01)?;
        log::trace!(
            "using endpoint 0x01 with buffer_size: {buffer_size}, num_transfers: {num_transfers:?}, timeout: {timeout:?}"
        );
        let mut writer = endpoint.writer(buffer_size);
        if let Some(t) = timeout {
            writer.set_write_timeout(t)
        }
        if let Some(n) = num_transfers {
            writer.set_num_transfers(n)
        }
        Ok(Self {
            dev,
            writer,
            buffer_size,
        })
    }

    pub fn mtu(&self) -> Result<usize> {
        Ok(self.buffer_size)
    }

    pub fn activate(&mut self) -> Result<()> {
        // self.dev.perform_format_config(BladeRfDirection::Rx, BladerfFormat::Sc16Q11)
        //    ?;
        self.dev.enable_module(BLADERF_MODULE_TX, true)
        // dev.experimental_control_urb()
    }

    pub fn deactivate(&mut self) -> Result<()> {
        //  self.dev.perform_format_deconfig(BladeRfDirection::Rx)?;
        self.dev.enable_module(BLADERF_MODULE_TX, false)
    }

    pub fn write(
        &mut self,
        _buffers: &[&[Complex32]],
        _at_ns: Option<i64>,
        _end_burst: bool,
        _timeout_us: i64,
    ) -> Result<usize> {
        // TODO: Revisit for correctness
        // https://doc.rust-lang.org/nightly/std/io/trait.Write.html#tymethod.write
        todo!()
    }

    pub fn write_all(
        &mut self,
        buffers: &[&[Complex32]],
        at_ns: Option<i64>,
        end_burst: bool,
        timeout_us: i64,
    ) -> Result<()> {
        // TODO, find out how to implement write_all
        // https://doc.rust-lang.org/nightly/std/io/trait.Write.html#method.write_all
        self.writer
            .set_write_timeout(Duration::from_micros(timeout_us as u64));
        if let Some(t) = at_ns {
            sleep(Duration::from_nanos(t as u64));
        }
        // let mut sent = 0;
        for (re, im) in buffers[0]
            .iter()
            // .map(|c| ((c.re * 2047.5) as i16, (c.im * 2047.5) as i16))
            .map(|c| (c.re as i16, c.im as i16))
        {
            let _ = self.writer.write(re.to_le_bytes().as_slice())?;
            let _ = self.writer.write(im.to_le_bytes().as_slice())?;
            // sent += 2 * size_of::<i16>();
        }
        if end_burst {
            self.writer.submit();
        }
        // Ok(sent)
        Ok(())
    }
}

impl BladeRf1 {
    // Perform the neccessary device configuration for the specified format
    // (e.g., enabling/disabling timestamp support), first checking that the
    // requested format would not conflict with the other stream direction.
    //
    //      dev     Device handle
    //      dir     Direction that is currently being configured
    //      format  Format the channel is being configured for
    //
    // @return 0 on success, BLADERF_ERR_* on failure
    pub fn perform_format_config(
        &self,
        dir: BladeRf1Direction,
        format: BladeRf1Format,
    ) -> Result<()> {
        // BladerfFormatPacketMeta
        // struct bladerf1_board_data *board_data = dev->board_data;

        // int status = 0;
        let mut use_timestamps: bool = false;
        let _other_using_timestamps: bool = false;

        // status = requires_timestamps(format, &use_timestamps);
        // if (status != 0) {
        //     log_debug("%s: Invalid format: %d\n", __FUNCTION__, format);
        //     return status;
        // }

        let _other = match dir {
            BladeRf1Direction::Rx => BladeRf1Direction::Tx,
            BladeRf1Direction::Tx => BladeRf1Direction::Rx,
        };

        // status = requires_timestamps(board_data->module_format[other],
        //     &other_using_timestamps);

        // if ((status == 0) && (other_using_timestamps != use_timestamps)) {
        //     log_debug("Format conflict detected: RX=%d, TX=%d\n");
        //     return BLADERF_ERR_INVAL;
        // }

        let mut gpio_val = self.config_gpio_read()?;

        log::debug!("gpio_val {gpio_val:#08x}");
        if format == BladeRf1Format::PacketMeta {
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

        self.config_gpio_write(gpio_val)?;
        // if (status == 0) {
        //     board_data->module_format[dir] = format;
        // }

        // return status;
        Ok(())
    }

    /// Deconfigure and update any state pertaining what a format that a stream
    /// direction is no longer using.
    ///
    ///    dev     Device handle
    ///    dir     Direction that is currently being deconfigured
    ///
    /// @return 0 on success, BLADERF_ERR_* on failure
    pub fn perform_format_deconfig(&self, direction: BladeRf1Direction) -> Result<()> {
        // struct bladerf1_board_data *board_data = dev->board_data;

        match direction {
            BladeRf1Direction::Rx | BladeRf1Direction::Tx => {
                // We'll reconfigure the HW when we call perform_format_config, so
                // we just need to update our stored information
                // board_data -> module_format[dir] = - 1;
            }
        }

        Ok(())
    }

    pub fn experimental_control_urb(&self) -> Result<()> {
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
            .wait()?;
        log::debug!("Control Response Data: {vec:?}");
        Ok(())
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

    // pub  fn _run_stream(&self) -> Result<()> {
    //     // TODO: In_ENDPOINT is 0x81 here, not 0x82
    //     let mut ep_bulk_in = self.interface.endpoint::<Bulk, In>(0x81)?;
    //
    //     let n_transfers = 8;
    //     let factor = 32;
    //     // let factor = match self.device.speed().unwrap_or(Speed::Low) {
    //     //     // TODO: These numbers are completely made up.
    //     //     // TODO: They should be based on real USB Frame sizes depending on the given Speed
    //     //     Speed::Low => 8,
    //     //     Speed::Full => 16,
    //     //     Speed::High => 32,
    //     //     Speed::Super => 32, // This factor is used by the original libusb libbladerf implementation.
    //     //     Speed::SuperPlus => 96,
    //     //     _ => 8,
    //     // };
    //
    //     let max_packet_size = ep_bulk_in.max_packet_size();
    //     let max_frame_size = max_packet_size/// factor;
    //     log::debug!("Max Packet Size: {max_packet_size}");
    //
    //     for _i in 0..n_transfers {
    //         let buffer = ep_bulk_in.allocate(max_frame_size);
    //         ep_bulk_in.submit(buffer);
    //         // log::debug!("submitted_transfers: {i}");
    //     }
    //
    //     loop {
    //         let result = ep_bulk_in.next_complete();
    //         log::debug!("{result:?}");
    //         if result.status.is_err() {
    //             break;
    //         }
    //         ep_bulk_in.submit(result.buffer);
    //     }
    //     Ok(())
    // }

    // pub  fn bladerf1_stream(&self, stream: &bladerf_stream, layout: BladeRfChannelLayout) -> Result<()> {
    //     let dir: BladeRfDirection = layout & BLADERF_DIRECTION_MASK;
    //     let stream_status: i32;
    //
    //     // if layout != BladeRfChannelLayout::BladerfRxX1 && layout != BladeRfChannelLayout::BladerfTxX1 {
    //     //     return Err(anyhow!("Invalid ChannelLayout"));
    //     // }
    //
    //     self.perform_format_config(dir, stream->format)?;
    //
    //     stream_status = self._run_stream(stream, layout);
    //     // TODO: static void LIBUSB_CALL lusb_stream_cb
    //
    //     self.perform_format_deconfig(dir)?;
    // }
}
