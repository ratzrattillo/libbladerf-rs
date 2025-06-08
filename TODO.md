- Implement schedule_retune for 
  FPGA Tuning mode for use in pub fn set_frequency in bladerf1.rs
- Move to thiserror for structured errors

// Get BladeRf frequency
// https://github.com/Nuand/bladeRF/blob/master/host/libraries/libbladeRF/include/libbladeRF.h#L694
// const BLADERF_CHANNEL_RX(ch) (bladerf_channel)(((ch) << 1) | 0x0)
// https://github.com/Nuand/bladeRF/blob/master/host/libraries/libbladeRF/include/libbladeRF.h#L694
// const BLADERF_MODULE_RX BLADERF_CHANNEL_RX(0)
// https://github.com/Nuand/bladeRF/blob/fe3304d75967c88ab4f17ff37cb5daf8ff53d3e1/host/libraries/libbladeRF/src/board/bladerf1/bladerf1.c#L2121
// static int bladerf1_get_frequency(struct bladerf *dev, bladerf_channel ch, bladerf_frequency *frequency);
// https://github.com/Nuand/bladeRF/blob/master/fpga_common/src/lms.c#L1698
// int lms_get_frequency(struct bladerf *dev, bladerf_module mod, struct lms_freq *f)
// lms_freq struct: https://github.com/Nuand/bladeRF/blob/master/fpga_common/include/lms.h#L101
// https://github.com/Nuand/bladeRF/blob/master/fpga_common/src/lms.c#L1698
// const uint8_t base = (mod == BLADERF_MODULE_RX) ? 0x20 : 0x10;
// sudo usermod -a -G wireshark jl
// sudo modprobe usbmon
// sudo setfacl -m u:jl:r /dev/usbmon*
// Wireshark Display filter depending on lsusb output: usb.bus_id == 2 and usb.device_address == 6