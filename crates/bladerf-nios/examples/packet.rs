use bladerf_nios::packet_generic::{NiosReq32x32, NiosResp32x32};

fn main() {
    type PktType = NiosReq32x32;

    // Create a new 32x32 NIOSII packet
    let mut packet = PktType::new(1, PktType::FLAG_WRITE, 3, 4);

    // Print debug output of a newly created packet
    println!("{packet:#?}");

    // Print display output of a newly created packet
    println!("{packet}");

    // Get pointer to underlying buffer
    //let _ptr = packet.as_mut_ptr();

    // Set individual field of a packet
    packet.set_target_id(0x33);

    // Check if a valid packet has been created:
    // packet.validate().expect("Failed to validate");

    // Convert a packet into a vector (underlying buffer is reused)
    let packet_vec: Vec<u8> = packet.into();

    // Convert a vector back into a packet
    let resp_packet = NiosResp32x32::from(packet_vec);

    // Get individual field of a packet
    let _target_id = resp_packet.target_id();
    
    // Check if packet indicates success
    let _success = resp_packet.is_success();

    // Check if a packet defines write or read operation
    let _is_write = resp_packet.is_write();
}
