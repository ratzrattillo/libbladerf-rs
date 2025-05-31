use bladerf_globals::BLADERF_MODULE_TX;
use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest, Tune};

fn main() {
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
}
