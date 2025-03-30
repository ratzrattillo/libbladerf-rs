#![allow(dead_code)]

pub const NIOS_PKT_8X8_MAGIC: u8 = 0x41; // 'A'
pub const NIOS_PKT_8X16_MAGIC: u8 = 0x42; // 'B'
pub const NIOS_PKT_8X32_MAGIC: u8 = 0x43; // 'C'
pub const NIOS_PKT_8X64_MAGIC: u8 = 0x44; // 'D'
pub const NIOS_PKT_16X64_MAGIC: u8 = 0x45; // 'E'
pub const NIOS_PKT_32X32_MAGIC: u8 = 0x4B; // 'K'

// pub const NIOS_PKT_LEGACY_MAGIC: u8 = 0x4E; // 'N'

/* Flag bits */
pub const NIOS_PKT_FLAG_READ: u8 = 0x0;
pub const NIOS_PKT_FLAG_WRITE: u8 = 0x1;
pub const NIOS_PKT_FLAG_SUCCESS: u8 = 0x2;

/* Request packet indices */
pub const NIOS_PKT_IDX_MAGIC: usize = 0x0;
pub const NIOS_PKT_IDX_TARGET_ID: usize = 0x1;
pub const NIOS_PKT_IDX_FLAGS: usize = 0x2;
pub const NIOS_PKT_IDX_ADDR: usize = 0x4;
//pub (crate) const NIOS_PKT_8X8_IDX_DATA: usize = 0x5;

/* IDs 0x80 through 0xff will not be assigned by Nuand. These are reserved
 * for user customizations */
pub const NIOS_PKT_TARGET_USR1: u8 = 0x80;
pub const NIOS_PKT_TARGET_USR128: u8 = 0xff;

/* Target IDs */
pub const NIOS_PKT_8X8_TARGET_LMS6: u8 = 0x00; /* LMS6002D register access */
pub const NIOS_PKT_8X8_TARGET_SI5338: u8 = 0x01; /* Si5338 register access */
pub const NIOS_PKT_8X8_TARGET_VCTCXO_TAMER: u8 = 0x02; /* VCTCXO Tamer control */
pub const NIOS_PKT_8X8_TX_TRIGGER_CTL: u8 = 0x03; /* TX trigger control */
pub const NIOS_PKT_8X8_RX_TRIGGER_CTL: u8 = 0x04; /* RX trigger control */

/* Target IDs */
pub const NIOS_PKT_8X16_TARGET_VCTCXO_DAC: u8 = 0x00;
pub const NIOS_PKT_8X16_TARGET_IQ_CORR: u8 = 0x01;
pub const NIOS_PKT_8X16_TARGET_AGC_CORR: u8 = 0x02;
pub const NIOS_PKT_8X16_TARGET_AD56X1_DAC: u8 = 0x03;
pub const NIOS_PKT_8X16_TARGET_INA219: u8 = 0x04;

/* Sub-addresses for the IQ Correction target block */
pub const NIOS_PKT_8X16_ADDR_IQ_CORR_RX_GAIN: u8 = 0x00;
pub const NIOS_PKT_8X16_ADDR_IQ_CORR_RX_PHASE: u8 = 0x01;
pub const NIOS_PKT_8X16_ADDR_IQ_CORR_TX_GAIN: u8 = 0x02;
pub const NIOS_PKT_8X16_ADDR_IQ_CORR_TX_PHASE: u8 = 0x03;

/* Sub-addresses for the AGC DC Correction target block */
pub const NIOS_PKT_8X16_ADDR_AGC_DC_Q_MAX: u8 = 0x00;
pub const NIOS_PKT_8X16_ADDR_AGC_DC_I_MAX: u8 = 0x01;
pub const NIOS_PKT_8X16_ADDR_AGC_DC_Q_MID: u8 = 0x02;
pub const NIOS_PKT_8X16_ADDR_AGC_DC_I_MID: u8 = 0x03;
pub const NIOS_PKT_8X16_ADDR_AGC_DC_Q_MIN: u8 = 0x04;
pub const NIOS_PKT_8X16_ADDR_AGC_DC_I_MIN: u8 = 0x05;
/* Target IDs */
pub const NIOS_PKT_8X32_TARGET_VERSION: u8 = 0x00; /* FPGA version (read only) */
pub const NIOS_PKT_8X32_TARGET_CONTROL: u8 = 0x01; /* FPGA control/config register */
pub const NIOS_PKT_8X32_TARGET_ADF4351: u8 = 0x02; /* XB-200 ADF4351 register access (write-only) */
pub const NIOS_PKT_8X32_TARGET_RFFE_CSR: u8 = 0x03; /* RFFE control & status GPIO */
pub const NIOS_PKT_8X32_TARGET_ADF400X: u8 = 0x04; /* ADF400x config */
pub const NIOS_PKT_8X32_TARGET_FASTLOCK: u8 = 0x05; /* Save AD9361 fast lock profile
 * to Nios */

/* Target IDs */

pub const NIOS_PKT_8X64_TARGET_TIMESTAMP: u8 = 0x00; /* Timestamp readback (read only) */

/* Sub-addresses for timestamp target */
pub const NIOS_PKT_8X64_TIMESTAMP_RX: u8 = 0x00;
pub const NIOS_PKT_8X64_TIMESTAMP_TX: u8 = 0x01;

/* Target IDs */
pub const NIOS_PKT_16X64_TARGET_AD9361: u8 = 0x00;
pub const NIOS_PKT_16X64_TARGET_RFIC: u8 = 0x01; /* RFIC control */

/* Target IDs */

/* For the EXP and EXP_DIR targets, the address is a bitmask of values
 * to read/write */
pub const NIOS_PKT_32X32_TARGET_EXP: u8 = 0x00; /* Expansion I/O */
pub const NIOS_PKT_32X32_TARGET_EXP_DIR: u8 = 0x01; /* Expansion I/O Direction reg */
pub const NIOS_PKT_32X32_TARGET_ADI_AXI: u8 = 0x02; /* ADI AXI Interface */
pub const NIOS_PKT_32X32_TARGET_WB_MSTR: u8 = 0x03; /* Wishbone Master */
