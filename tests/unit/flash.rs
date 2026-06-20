use libbladerf_rs::flash::{
    BLADERF_FLASH_FPGA_SIZE_40KLE, BLADERF_FLASH_FPGA_SIZE_115KLE, FpgaSize, binkv_add_field,
    binkv_decode_field, binkv_encode_field, is_valid_fpga_size, make_cal_region, zcrc,
};

#[test]
fn test_zcrc_empty() {
    assert_eq!(zcrc(&[]), 0);
}

#[test]
fn test_zcrc_known() {
    // CRC16-CCITT with init=0 (matching C zcrc()) of "123456789" is 0x31C3
    // Note: 0x29B1 is CRC-CCITT-FALSE (init=0xFFFF), which is different
    assert_eq!(zcrc(b"123456789"), 0x31C3);
}

#[test]
fn test_binkv_roundtrip() {
    let mut buf = [0xFFu8; 256];
    binkv_add_field(&mut buf, "B", "40").unwrap();
    binkv_add_field(&mut buf, "DAC", "32768").unwrap();

    assert_eq!(binkv_decode_field(&buf, "B").unwrap(), "40");
    assert_eq!(binkv_decode_field(&buf, "DAC").unwrap(), "32768");
}

#[test]
fn test_binkv_all_ff() {
    let buf = [0xFFu8; 256];
    assert!(binkv_decode_field(&buf, "B").is_err());
}

#[test]
fn test_binkv_encode_decode_single() {
    let mut buf = [0xFFu8; 256];
    let idx = binkv_encode_field(&mut buf, 0, "DAC", "1000").unwrap();
    assert!(idx > 0);
    assert_eq!(binkv_decode_field(&buf, "DAC").unwrap(), "1000");
}

#[test]
fn test_make_cal_region() {
    let cal = make_cal_region(FpgaSize::KLE40, 0x8000).unwrap();
    assert_eq!(binkv_decode_field(&cal, "B").unwrap(), "40");
    assert_eq!(binkv_decode_field(&cal, "DAC").unwrap(), "32768");
}

#[test]
fn test_fpga_size_roundtrip() {
    for (variant, s) in [
        (FpgaSize::KLE40, "40"),
        (FpgaSize::KLE115, "115"),
        (FpgaSize::A4, "A4"),
        (FpgaSize::A5, "A5"),
        (FpgaSize::A9, "A9"),
    ] {
        assert_eq!(variant.as_str(), s);
        assert_eq!(FpgaSize::parse(s).unwrap(), variant);
    }
    assert!(FpgaSize::parse("unknown").is_err());
}

#[test]
fn test_is_valid_fpga_size() {
    assert!(is_valid_fpga_size(BLADERF_FLASH_FPGA_SIZE_40KLE));
    assert!(is_valid_fpga_size(BLADERF_FLASH_FPGA_SIZE_115KLE));
    assert!(!is_valid_fpga_size(0));
    assert!(!is_valid_fpga_size(BLADERF_FLASH_FPGA_SIZE_40KLE - 1));
    assert!(!is_valid_fpga_size(BLADERF_FLASH_FPGA_SIZE_40KLE + 1));
}
