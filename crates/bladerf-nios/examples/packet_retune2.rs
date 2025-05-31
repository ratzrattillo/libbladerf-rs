use bladerf_globals::BLADERF_MODULE_RX;
use bladerf_nios::packet_retune2::NiosPktRetune2Request;

fn main() {
    let pkt = NiosPktRetune2Request::new(BLADERF_MODULE_RX, u64::MIN, 0xffff, 0xff, 0xff, 0xff);
    let vec: Vec<u8> = pkt.into();
    println!("{vec:x?}");
}
