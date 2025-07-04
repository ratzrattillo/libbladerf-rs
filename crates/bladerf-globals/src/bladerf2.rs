#![allow(dead_code)]
pub struct BladeRf2QuickTune {
    /**< Profile number in Nios */
    nios_profile: u16,
    /**< Profile number in RFFE */
    rffe_profile: u8,
    /**< RFFE port settings */
    port: u8,
    /**< External SPDT settings */
    spdt: u8,
}
