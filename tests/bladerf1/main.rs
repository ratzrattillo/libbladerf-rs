#[path = "../common/mod.rs"]
mod common;

mod bandwidth;
mod correction;
mod dc_cal_table;
mod dc_calibration;
mod flash;
mod fpga;
mod frequency;
mod gain;
mod loopback;
mod open;
mod rx_mux;
mod sample_rate;
#[cfg(feature = "xb200")]
mod xb200;
#[cfg(feature = "xb200")]
mod xb200_frequency;
