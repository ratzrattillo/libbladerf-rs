// #[allow(dead_code)]
//
// pub(crate) const NIOS_PKT_LEGACY_MAGIC: u8 = 0x4E; // 'N'
//
// pub(crate) const NIOS_PKT_LEGACY_DEV_GPIO_ADDR: u8 = 0x0;
//
// pub(crate) const NIOS_PKT_LEGACY_DEV_RX_GAIN_ADDR: u8 = 0x4;
// pub(crate) const NIOS_PKT_LEGACY_DEV_RX_PHASE_ADDR: u8 = 0x6;
// pub(crate) const NIOS_PKT_LEGACY_DEV_TX_GAIN_ADDR: u8 = 0x8;
// pub(crate) const NIOS_PKT_LEGACY_DEV_TX_PHASE_ADDR: u8 = 0xa;
// pub(crate) const NIOS_PKT_LEGACY_DEV_FPGA_VERSION_ID: u8 = 0xc;
//
// pub(crate) const NIOS_PKT_LEGACY_MODE_CNT_MASK: u8 = 0x7;
// pub(crate) const NIOS_PKT_LEGACY_MODE_CNT_SHIFT: u8 = 0x0;
// pub(crate) const NIOS_PKT_LEGACY_MODE_DEV_MASK: u8 = 0x30;
// pub(crate) const NIOS_PKT_LEGACY_MODE_DEV_SHIFT: u8 = 0x4;
//
// pub(crate) const NIOS_PKT_LEGACY_DEV_CONFIG: u8 = 0 << NIOS_PKT_LEGACY_MODE_DEV_SHIFT;
// pub(crate) const NIOS_PKT_LEGACY_DEV_LMS: u8 = 1 << NIOS_PKT_LEGACY_MODE_DEV_SHIFT;
// pub(crate) const NIOS_PKT_LEGACY_DEV_VCTCXO: u8 = 2 << NIOS_PKT_LEGACY_MODE_DEV_SHIFT;
// pub(crate) const NIOS_PKT_LEGACY_DEV_SI5338: u8 = 3 << NIOS_PKT_LEGACY_MODE_DEV_SHIFT;
//
// pub(crate) const NIOS_PKT_LEGACY_MODE_DIR_MASK: u8 = 0xc0;
// pub(crate) const NIOS_PKT_LEGACY_MODE_DIR_SHIFT: u8 = 0x6;
// pub(crate) const NIOS_PKT_LEGACY_MODE_DIR_READ: u8 = 2 << NIOS_PKT_LEGACY_MODE_DIR_SHIFT;
// pub(crate) const NIOS_PKT_LEGACY_MODE_DIR_WRITE: u8 = 1 << NIOS_PKT_LEGACY_MODE_DIR_SHIFT;
//
// /* PIO address space */
//
// /*
//  * 32-bit Device control register.
//  *
//  * This is register accessed via the libbladeRF functions,
//  * bladerf_config_gpio_write() and bladerf_config_gpio_read().
//  */
// pub(crate) const NIOS_PKT_LEGACY_PIO_ADDR_CONTROL: u8 = 0x0;
// pub(crate) const NIOS_PKT_LEGACY_PIO_LEN_CONTROL: u8 = 0x4;
//
// /*
//  * IQ Correction: 16-bit RX Gain value
//  */
// pub(crate) const NIOS_PKT_LEGACY_PIO_ADDR_IQ_RX_GAIN: u8 = 0x4;
// pub(crate) const NIOS_PKT_LEGACY_PIO_LEN_IQ_RX_GAIN: u8 = 0x2;
//
// /*
//  * IQ Correction: 16-bit RX Phase value
//  */
// pub(crate) const NIOS_PKT_LEGACY_PIO_ADDR_IQ_RX_PHASE: u8 = 0x6;
// pub(crate) const NIOS_PKT_LEGACY_PIO_LEN_IQ_RX_PHASE: u8 = 0x2;
//
// /*
//  * IQ Correction: 16-bit TX Gain value
//  */
// pub(crate) const NIOS_PKT_LEGACY_PIO_ADDR_IQ_TX_GAIN: u8 = 0x8;
// pub(crate) const NIOS_PKT_LEGACY_PIO_LEN_IQ_TX_GAIN: u8 = 0x2;
//
// /*
//  * IQ Correction: 16-bit TX Phase value
//  */
// pub(crate) const NIOS_PKT_LEGACY_PIO_ADDR_IQ_TX_PHASE: u8 = 0xa;
// pub(crate) const NIOS_PKT_LEGACY_PIO_LEN_IQ_TX_PHASE: u8 = 0x2;
//
// /*
//  * 32-bit FPGA Version (read-only)
//  */
// pub(crate) const NIOS_PKT_LEGACY_PIO_ADDR_FPGA_VERSION: u8 = 0xc;
// pub(crate) const NIOS_PKT_LEGACY_PIO_LEN_FPGA_VERSION: u8 = 0x4;
//
// /*
//  * 64-bit RX timestamp
//  */
// pub(crate) const NIOS_PKT_LEGACY_PIO_ADDR_RX_TIMESTAMP: u8 = 0x10;
// pub(crate) const NIOS_PKT_LEGACY_PIO_LEN_RX_TIMESTAMP: u8 = 0x8;
//
// /*
//  * 64-bit TX timestamp
//  */
// pub(crate) const NIOS_PKT_LEGACY_PIO_ADDR_TX_TIMESTAMP: u8 = 0x18;
// pub(crate) const NIOS_PKT_LEGACY_PIO_LEN_TX_TIMESTAMP: u8 = 0x8;
//
// /*
//  * VCTCXO Trim DAC value
//  */
// pub(crate) const NIOS_PKT_LEGACY_PIO_ADDR_VCTCXO: u8 = 0x22;
// pub(crate) const NIOS_PKT_LEGACY_PIO_LEN_VCTCXO: u8 = 0x2;
//
// /*
//  * XB-200 ADF4351 Synthesizer
//  */
// pub(crate) const NIOS_PKT_LEGACY_PIO_ADDR_XB200_SYNTH: u8 = 0x24;
// pub(crate) const NIOS_PKT_LEGACY_PIO_LEN_XB200_SYNTH: u8 = 0x4;
//
// /*
//  * Expansion IO
//  */
// pub(crate) const NIOS_PKT_LEGACY_PIO_ADDR_EXP: u8 = 0x28;
// pub(crate) const NIOS_PKT_LEGACY_PIO_LEN_EXP: u8 = 0x4;
//
// /*
//  * Expansion IO Direction
//  */
// pub(crate) const NIOS_PKT_LEGACY_PIO_ADDR_EXP_DIR: u8 = 0x2C;
// pub(crate) const NIOS_PKT_LEGACY_PIO_LEN_EXP_DIR: u8 = 0x4;
