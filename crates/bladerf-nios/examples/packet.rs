use bladerf_nios::packet_generic::NiosPkt32x32;

fn main() {
    type PktType = NiosPkt32x32;

    // Create a new 32x32 NIOSII packet
    let packet = PktType::new(1, 2, 3, 4);

    // Print debug output of a newly created packet
    println!("{packet:#?}");

    // Print display output of a newly created packet
    println!("{packet}");

    // Get pointer to underlying buffer
    //let _ptr = packet.as_mut_ptr();

    // Convert a packet into a vector (underlying buffer is reused)
    let packet_vec: Vec<u8> = packet.into();

    // Convert a vector back into a packet
    let mut reused_packet = PktType::from(packet_vec);

    // Check if a valid packet has been created:
    // reused_packet.validate().expect("Failed to validate");

    // Get individual field of a packet
    let _target_id = reused_packet.target_id();

    // Set individual field of a packet
    reused_packet.set_target_id(0x33);

    // Check if packet indicates success
    let _success = reused_packet.is_success();

    // Check if a packet defines write or read operation
    let _is_write = reused_packet.is_write();
}
