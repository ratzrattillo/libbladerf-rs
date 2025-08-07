use std::fmt::{Display, Formatter};

/// BladeRF1 USB vendor ID.
pub const BLADERF1_USB_VID: u16 = 0x2CF0;
/// BladeRF1 USB product ID.
pub const BLADERF1_USB_PID: u16 = 0x5246;

/// Minimum sample rate, in Hz.
///
/// \deprecated Use bladerf_get_sample_rate_range()
pub const BLADERF_SAMPLERATE_MIN: u32 = 80000;

/// Maximum recommended sample rate, in Hz.
///
/// \deprecated Use bladerf_get_sample_rate_range()
pub const BLADERF_SAMPLERATE_REC_MAX: u32 = 40000000;

/// Minimum bandwidth, in Hz
///
/// \deprecated Use bladerf_get_bandwidth_range()
pub const BLADERF_BANDWIDTH_MIN: u32 = 1500000;

/// Maximum bandwidth, in Hz
///
/// \deprecated Use bladerf_get_bandwidth_range()
pub const BLADERF_BANDWIDTH_MAX: u32 = 28000000;

/// Minimum tunable frequency (with an XB-200 attached), in Hz.
///
/// While this value is the lowest permitted, note that the components on the
/// XB-200 are only rated down to 50 MHz. Be aware that performance will likely
/// degrade as you tune to lower frequencies.
///
/// \deprecated Call bladerf_expansion_attach(), then use
///             bladerf_get_frequency_range() to get the frequency range.
pub const BLADERF_FREQUENCY_MIN_XB200: u32 = 0;

/// Minimum tunable frequency (without an XB-200 attached), in Hz
///
/// \deprecated Use bladerf_get_frequency_range()
pub const BLADERF_FREQUENCY_MIN: u32 = 237500000;

/// Maximum tunable frequency, in Hz
///
/// \deprecated Use bladerf_get_frequency_range()
pub const BLADERF_FREQUENCY_MAX: u32 = 3800000000;

// /// @ingroup FN_IMAGE
// /// @defgroup BLADERF_FLASH_CONSTANTS Flash image format constants
// ///
// /// note These apply to both the bladeRF1 and bladeRF2, but they are still in
// ///       bladeRF1.h for the time being.

/// Byte address of FX3 firmware
pub const BLADERF_FLASH_ADDR_FIRMWARE: u32 = 0x00000000;

/// Length of firmware region of flash, in bytes
pub const BLADERF_FLASH_BYTE_LEN_FIRMWARE: u32 = 0x00030000;

/// Byte address of calibration data region
pub const BLADERF_FLASH_ADDR_CAL: u32 = 0x00030000;

/// Length of calibration data, in bytes
pub const BLADERF_FLASH_BYTE_LEN_CAL: u16 = 0x100;

/// Byte address of the autoloaded FPGA and associated metadata.
///
/// The first page is allocated for metadata, and the FPGA bitstream resides
/// in the following pages.
pub const BLADERF_FLASH_ADDR_FPGA: u32 = 0x00040000;

/// Stream channel layout
#[derive(PartialEq)]
pub enum BladeRf1ChannelLayout {
    RxX1 = 0, // x1 RX (SISO)
    TxX1 = 1, // x1 TX (SISO)
    RxX2 = 2, // x2 RX (MIMO)
    TxX2 = 3, // x2 TX (MIMO)
}

/// RX mux modes
#[derive(PartialEq, Debug, Clone)]
pub enum BladeRf1RxMux {
    MuxInvalid = -1,
    MuxBaseband = 0,
    Mux12BitCounter = 1,
    Mux32BitCounter = 2,
    MuxDigitalLoopback = 4,
}

impl From<u32> for BladeRf1RxMux {
    fn from(value: u32) -> Self {
        match value {
            0 => BladeRf1RxMux::MuxBaseband,
            1 => BladeRf1RxMux::Mux12BitCounter,
            2 => BladeRf1RxMux::Mux32BitCounter,
            4 => BladeRf1RxMux::MuxDigitalLoopback,
            _ => BladeRf1RxMux::MuxInvalid,
        }
    }
}

/// Enable LMS receive
///
/// @note This bit is set/cleared by bladerf_enable_module()
pub const BLADERF_GPIO_LMS_RX_ENABLE: u8 = 1 << 1;

/// Enable LMS transmit
///
/// @note This bit is set/cleared by bladerf_enable_module()
pub const BLADERF_GPIO_LMS_TX_ENABLE: u8 = 1 << 2;

/// Switch to use TX low band (300MHz - 1.5GHz)
///
/// @note This is set using bladerf_set_frequency().
pub const BLADERF_GPIO_TX_LB_ENABLE: u8 = 2 << 3;

/// Switch to use TX high band (1.5GHz - 3.8GHz)
///
/// @note This is set using bladerf_set_frequency().
pub const BLADERF_GPIO_TX_HB_ENABLE: u8 = 1 << 3;

/// Counter mode enable
///
/// Setting this bit to 1 instructs the FPGA to replace the (I, Q) pair in sample
/// data with an incrementing, little-endian, 32-bit counter value. A 0 in bit
/// specifies that sample data should be sent (as normally done).
///
/// This feature is useful when debugging issues involving dropped samples.
pub const BLADERF_GPIO_COUNTER_ENABLE: u16 = 1 << 9;

/// Bit mask representing the rx mux selection
///
/// @note These bits are set using bladerf_set_rx_mux()
pub const BLADERF_GPIO_RX_MUX_MASK: u16 = 7 << BLADERF_GPIO_RX_MUX_SHIFT;

/// Starting bit index of the RX mux values in FX3 <-> FPGA GPIO bank
pub const BLADERF_GPIO_RX_MUX_SHIFT: u16 = 8;

/// Switch to use RX low band (300M - 1.5GHz)
///
/// @note This is set using bladerf_set_frequency().
pub const BLADERF_GPIO_RX_LB_ENABLE: u16 = 2 << 5;

/// Switch to use RX high band (1.5GHz - 3.8GHz)
///
/// @note This is set using bladerf_set_frequency().
pub const BLADERF_GPIO_RX_HB_ENABLE: u16 = 1 << 5;

/// This GPIO bit configures the FPGA to use smaller DMA transfers (256 cycles
/// instead of 512). This is required when the device is not connected at Super
/// Speed (i.e., when it is connected at High Speed).
///
/// However, the caller need not set this in bladerf_config_gpio_write() calls.
/// The library will set this as needed; callers generally do not need to be
/// concerned with setting/clearing this bit.
pub const BLADERF_GPIO_FEATURE_SMALL_DMA_XFER: u16 = 1 << 7;

/// Enable Packet mode
pub const BLADERF_GPIO_PACKET: u32 = 1 << 19;

/// Enable 8bit sample mode
pub const BLADERF_GPIO_8BIT_MODE: u32 = 1 << 20;

/// AGC enable control bit
///
/// @note This is set using bladerf_set_gain_mode().
pub const BLADERF_GPIO_AGC_ENABLE: u32 = 1 << 18;

/// Enable-bit for timestamp counter in the FPGA
pub const BLADERF_GPIO_TIMESTAMP: u32 = 1 << 16;

/// Timestamp 2x divider control.
///
/// @note <b>Important</b>: This bit has no effect and is always enabled (1) in
/// FPGA versions >= v0.3.0.
///
/// @note The remainder of the description of this bit is presented here for
/// historical purposes only. It is only relevant to FPGA versions <= v0.1.2.
///
/// By default, (value = 0), the sample counter is incremented with I and Q,
/// yielding two counts per sample.
///
/// Set this bit to 1 to enable a 2x timestamp divider, effectively achieving 1
/// timestamp count per sample.
///
pub const BLADERF_GPIO_TIMESTAMP_DIV2: u32 = 1 << 17;

/// Packet capable core present bit.
///
/// @note This is a read-only bit. The FPGA sets its value, and uses it to inform
///  host that there is a core capable of using packets in the FPGA.
pub const BLADERF_GPIO_PACKET_CORE_PRESENT: u32 = 1 << 28;

/// Maximum output frequency on SMB connector, if no expansion board attached.
pub const BLADERF_SMB_FREQUENCY_MAX: u32 = 200000000;

/// Minimum output frequency on SMB connector, if no expansion board attached.
pub const BLADERF_SMB_FREQUENCY_MIN: u32 = (38400000 * 66) / (32 * 567);

pub const BLADERF_DIRECTION_MASK: u8 = 0x1;

/// Expansion boards
#[derive(Clone, PartialEq, Debug)]
pub enum BladeRf1Xb {
    /// No expansion boards attached
    XbNone = 0,
    /// XB-100 GPIO expansion board.
    ///   This device is not yet supported in
    ///   libbladeRF, and is here as a placeholder
    ///   for future support.
    Xb100,
    /// XB-200 Transverter board
    Xb200,
    /// XB-300 Amplifier board
    Xb300,
}

/// Version structure for FPGA, firmware, libbladeRF, and associated utilities
#[derive(Debug)]
pub struct BladeRf1Version {
    /// Major version
    pub major: u16,
    /// Minor version
    pub minor: u16,
    /// Patch version
    pub patch: u16,
}

impl Display for BladeRf1Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}.{}.{}", self.major, self.minor, self.patch))
    }
}

/// Quick Re-tune parameters.
///
/// @note These parameters, which are associated with the RFIC's register values,
///       are sensitive to changes in the operating environment (e.g.,
///       temperature).
///
/// This structure should be filled in via bladerf_get_quick_tune().
pub struct BladeRf1QuickTune {
    /// Choice of VCO and VCO division factor
    pub freqsel: u8,
    /// VCOCAP value
    pub vcocap: u8,
    /// Integer portion of LO frequency value
    pub nint: u16,
    /// Fractional portion of LO frequency value
    pub nfrac: u32,
    /// Flag bits used internally by libbladeRF
    pub flags: u8,
    /// Flag bits used to configure XB
    pub xb_gpio: u8,
}

/// Correction parameter selection
///
/// These values specify the correction parameter to modify or query when calling
/// bladerf_set_correction() or bladerf_get_correction(). Note that the meaning
/// of the `value` parameter to these functions depends upon the correction
/// parameter.
#[derive(Clone, Debug)]
pub enum BladeRf1Correction {
    /// Adjusts the in-phase DC offset. Valid values are \[-2048, 2048\], which are
    /// scaled to the available control bits.
    DcoffI,

    /// Adjusts the quadrature DC offset. Valid values are \[-2048, 2048\], which
    /// are scaled to the available control bits.
    DcoffQ,

    /// Adjusts phase correction of \[-10, 10\] degrees, via a provided count value
    /// of \[-4096, 4096\].
    Phase,

    /// Adjusts gain correction value in \[-1.0, 1.0\], via provided values in the
    /// range of \[-4096, 4096\].
    Gain,
}
