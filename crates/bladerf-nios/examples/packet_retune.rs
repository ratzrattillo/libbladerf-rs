use bladerf_globals::BLADERF_MODULE_RX;
use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest, Tune};

fn main() -> anyhow::Result<()> {
    let mut v = vec![0u8; 4];

    let nint: u16 = 0x01ff;
    let nfrac: u32 = 0x007fffff;

    v[0] = ((nint >> 1) & 0xff) as u8;
    v[1] = ((nint & 0x1) << 7) as u8;

    println!("{v:x?}");

    v[1] |= ((nfrac >> 16) & 0x7f) as u8;
    v[2] = ((nfrac >> 8) & 0xff) as u8;
    v[3] = (nfrac & 0xff) as u8;

    println!("{v:x?}");

    let pkt = NiosPktRetuneRequest::new(
        BLADERF_MODULE_RX,
        u64::MIN,
        nint,
        nfrac,
        0x3f,
        0x3f,
        Band::Low,
        Tune::Normal,
        0x0,
    );
    let vec: Vec<u8> = pkt.into();
    println!("{vec:x?}");

    Ok(())
}
