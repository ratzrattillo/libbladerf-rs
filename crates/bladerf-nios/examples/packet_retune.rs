use bladerf_globals::BLADERF_MODULE_TX;
use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest, Tune};

fn main() -> anyhow::Result<()> {
    let plain_vec = vec![
        0x54, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3f, 0xb9, 0x55, 0x55, 0xac, 0x1f,
        0x00,
    ];
    println!("{plain_vec:x?}");

    let from_plain_vec = NiosPktRetuneRequest::from(plain_vec);
    println!("{from_plain_vec:?}");

    let from_new = NiosPktRetuneRequest::new(
        BLADERF_MODULE_TX,
        0,
        0x3FB,
        0x4AAAAB,
        0x2c,
        0x1f,
        Band::High,
        Tune::Normal,
        0x00,
    );
    println!("{from_new:?}");

    let from_new_vec: Vec<u8> = from_new.into();
    println!("{from_new_vec:x?}");

    // println!("{f:?}");

    // let band = if f.flags & LMS_FREQ_FLAGS_LOW_BAND != 0 {
    // Band::Low
    // } else {
    // Band::High
    // };
    //
    // let tune = if (f.flags & LMS_FREQ_FLAGS_FORCE_VCOCAP) != 0 {
    // Tune::Quick
    // } else {
    // Tune::Normal
    // };

    //
    // let mut v = vec![0u8; 16];
    //
    // // LmsFreq { freqsel: 44, vcocap: 31, nint: 1019, nfrac: 4893355, flags: 0, xb_gpio: 0, x: 16, vcocap_result: 0 }
    // let nint: u16 = 1019;
    // let nfrac: u32 = 4893355;
    // let freqsel: u8 = 44;
    // let vcocap: u8 = 31;
    //
    // const IDX_MAGIC: usize = 0;
    // const IDX_TIME: usize = 1;
    // const IDX_INTFRAC: usize = 9;
    // const IDX_FREQSEL: usize = 13;
    // const IDX_BANDSEL: usize = 14;
    // const IDX_RESV: usize = 15;
    //
    // v[IDX_MAGIC] = 0x54;
    //
    // v[IDX_INTFRAC] = (nint >> 1) as u8;
    // v[IDX_INTFRAC + 1] = ((nint & 0x1) << 7) as u8;
    //
    // println!("{v:x?}");
    //
    // v[IDX_INTFRAC + 1] |= (nfrac >> 16) as u8;
    // v[IDX_INTFRAC + 2] = (nfrac >> 8) as u8;
    // v[IDX_INTFRAC + 3] = nfrac as u8;
    //
    // println!("{v:x?}");
    //
    // v[IDX_FREQSEL] = freqsel;
    //
    // println!("{v:x?}");
    //
    // let pkt = NiosPktRetuneRequest::new(
    //     BLADERF_MODULE_RX,
    //     u64::MIN,
    //     nint,
    //     nfrac,
    //     0x3f,
    //     0x3f,
    //     Band::Low,
    //     Tune::Normal,
    //     0x0,
    // );
    // let vec: Vec<u8> = pkt.into();
    // println!("{vec:x?}");

    Ok(())
}
