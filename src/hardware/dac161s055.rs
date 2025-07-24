use crate::Result;
use crate::nios::Nios;
use bladerf_nios::NIOS_PKT_8X16_TARGET_VCTCXO_DAC;
use nusb::Interface;

#[derive(Clone)]
pub struct DAC161S055 {
    interface: Interface,
}

/// The DAC161S055 is a precision 16-bit, buffered
/// voltage output Digital-to-Analog Converter (DAC) that
/// operates from a 2.7V to 5.25V supply with a separate
/// I/O supply pin that operates down to 1.7V. The on-
/// chip precision output buffer provides rail-to-rail output
/// swing and has a typical settling time of 5 μsec. The
/// external voltage reference can be set between 2.5V
/// and VA (the analog supply voltage), providing the
/// widest dynamic output range possible.
///
/// The 4-wire SPI compatible interface operates at clock
/// rates up to 20 MHz. The part is capable of Diasy
/// Chain and Data Read Back. An onboard power-on-
/// reset (POR) circuit ensures the output powers up to a
/// known state.
///
/// The DAC161S055 features a power-up value pin
/// (MZB), a load DAC pin (LDACB) and a DAC clear
/// (CLRB) pin. MZB sets the startup output voltage to
/// either GND or mid-scale. LDACB updates the output
/// allowing multiple DACs to update their outputs
/// simultaneously. CLRB can be used to reset the
/// output signal to the value determined by MZB.
///
/// The DAC161S055 has a power-down option that
/// reduces power consumption when the part is not in
/// use. It is available in a 16-lead WQFN package.
///
/// FEATURES
/// • 16-bit DAC with a Two-buffer SPI Interface
/// • Asynchronous Load DAC and Reset Pins
/// • Compatibility with 1.8V Controllers
/// • Buffered Voltage Output with Rail-to-Rail Capability
/// • Wide Voltage Reference Range of +2.5V to VA
/// • Wide Temperature Range of −40°C to +105°C
/// • Packaged in a 16-pin WQFN
///
/// APPLICATIONS
/// • Process Control known state.
/// • Automatic Test Equipment
/// • Programmable Voltage Sources
/// • Communication Systems.
/// • Data Acquisition
/// • Industrial PLCs simultaneously.
/// • Portable Battery Powered Instruments
///
/// KEY SPECIFICATIONS
/// • Resolution (Specified Monotonic) 16 bits
/// • INL ±3 LSB (max)
/// • Very Low Output Noise 120 nV/√Hz (typ)
/// • Glitch Impulse 7 nV-s (typ)
/// • Output Settling Time 5 μs (typ)
/// • Power Consumption 5.5 mW at 5.25 V (max)
impl DAC161S055 {
    pub fn new(interface: Interface) -> Self {
        Self { interface }
    }

    pub fn write(&self, value: u16) -> Result<()> {
        // Ensure the device is in write-through mode
        self.interface
            .nios_write::<u8, u16>(NIOS_PKT_8X16_TARGET_VCTCXO_DAC, 0x28, 0x0)?;

        // Write DAC value to channel 0
        self.interface
            .nios_write::<u8, u16>(NIOS_PKT_8X16_TARGET_VCTCXO_DAC, 0x8, value)
    }
}
