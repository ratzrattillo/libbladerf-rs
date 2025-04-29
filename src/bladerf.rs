#[macro_export]
macro_rules! bladerf_channel_rx {
    ($ch:expr) => {
        ((($ch) << 1) | 0x0) as u8
    };
}
#[macro_export]
macro_rules! bladerf_channel_tx {
    ($ch:expr) => {
        ((($ch) << 1) | 0x1) as u8
    };
}

/**
 * Mapping of human-readable names to loopback modes
 */
pub struct BladerfLoopbackModes {
    /**< Name of loopback mode */
    _name: String,
    /**< Loopback mode enumeration */
    _mode: BladerfLoopback,
}

/**
* Loopback options
*/
#[derive(PartialEq)]
pub enum BladerfLoopback {
    /** Disables loopback and returns to normal operation. */
    BladerfLbNone = 0,

    /** Firmware loopback inside of the FX3 */
    BladerfLbFirmware,

    /** Baseband loopback. TXLPF output is connected to the RXVGA2 input. */
    BladerfLbBbTxlpfRxvga2,

    /** Baseband loopback. TXVGA1 output is connected to the RXVGA2 input. */
    BladerfLbBbTxvga1Rxvga2,

    /** Baseband loopback. TXLPF output is connected to the RXLPF input. */
    BladerfLbBbTxlpfRxlpf,

    /** Baseband loopback. TXVGA1 output is connected to RXLPF input. */
    BladerfLbBbTxvga1Rxlpf,

    /**
     * RF loopback. The TXMIX output, through the AUX PA, is connected to the
     * output of LNA1.
     */
    BladerfLbRfLna1,

    /**
     * RF loopback. The TXMIX output, through the AUX PA, is connected to the
     * output of LNA2.
     */
    BladerfLbRfLna2,

    /**
     * RF loopback. The TXMIX output, through the AUX PA, is connected to the
     * output of LNA3.
     */
    BladerfLbRfLna3,

    /** RFIC digital loopback (built-in self-test) */
    BladerfLbRficBist,
}

/**
 * Gain control modes
 *
 * In general, the default mode is automatic gain control. This will
 * continuously adjust the gain to maximize dynamic range and minimize clipping.
 *
 * @note Implementers are encouraged to simply present a boolean choice between
 *       "AGC On" (::BladerfGainDefault) and "AGC Off" (::BladerfGainMgc).
 *       The remaining choices are for advanced use cases.
 */
#[derive(PartialEq)]
pub enum BladerfGainMode {
    /** Device-specific default (automatic, when available)
     *
     * On the bladeRF x40 and x115 with FPGA versions >= v0.7.0, this is
     * automatic gain control.
     *
     * On the bladeRF 2.0 Micro, this is BladerfGainSlowattackAgc with
     * reasonable default settings.
     */
    BladerfGainDefault,

    /** Manual gain control
     *
     * Available on all bladeRF models.
     */
    BladerfGainMgc,

    /** Automatic gain control, fast attack (advanced)
     *
     * Only available on the bladeRF 2.0 Micro. This is an advanced option, and
     * typically requires additional configuration for ideal performance.
     */
    BladerfGainFastattackAgc,

    /** Automatic gain control, slow attack (advanced)
     *
     * Only available on the bladeRF 2.0 Micro. This is an advanced option, and
     * typically requires additional configuration for ideal performance.
     */
    BladerfGainSlowattackAgc,

    /** Automatic gain control, hybrid attack (advanced)
     *
     * Only available on the bladeRF 2.0 Micro. This is an advanced option, and
     * typically requires additional configuration for ideal performance.
     */
    BladerfGainHybridAgc,
}

#[allow(dead_code)]
pub const BLADERF_MODULE_RX: u8 = bladerf_channel_rx!(0);
#[allow(dead_code)]
pub const BLADERF_MODULE_TX: u8 = bladerf_channel_tx!(0);

pub trait BladeRf {
    //fn nios_send(&self, endpoint_in: u8, endpoint_out: u8, pkt: Vec<u8>) -> Result<Vec<u8>>;
}

#[derive(Clone, Default)]
pub struct BladerfRationalRate {
    /* Integer portion */
    pub integer: u64,
    /* Numerator in fractional portion */
    pub num: u64,
    /* Denominator in fractional portion. This must be greater than 0. */
    pub den: u64,
}

#[repr(u8)]
pub enum StringDescriptors {
    Manufacturer = 0x1, // Don't want to start with 0 as 0 is reserved for the language table
    Product,
    Serial,
    Fx3Firmware,
}

#[repr(u8)]
pub enum DescriptorTypes {
    Device = 0x01,
    Configuration = 0x2, // Don't want to start with 0 as 0 is reserved for the language table
    String = 0x03,
    Default = 0x06,
    BOS = 0x0f,
}
