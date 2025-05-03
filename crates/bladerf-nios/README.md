# LibNIOS-RS
Pure Rust library to create and manipulate NIOSII packets.

The packets created can be sent directly to e.g. a USB device.

## Features
Allows you to create NIOSII packets in the following formats:

| Name         | Addr-Width | Data-Width | Magic-Number |
|:-------------|------------|------------|--------------|
| NiosPkt8x8   | 8          | 8          | 0x41 ('A')   |
| NiosPkt8x16  | 8          | 16         | 0x42 ('B')   |
| NiosPkt8x32  | 8          | 32         | 0x43 ('C')   |
| NiosPkt8x64  | 8          | 64         | 0x44 ('D')   |
| NiosPkt16x64 | 16         | 64         | 0x45 ('E')   |
| NiosPkt32x32 | 32         | 32         | 0x4B ('K')   |

## Example
```rust
use anyhow::Result;
use libnios_rs::packet::NiosPkt32x32;

fn main() -> Result<()> {
  type PktType = NiosPkt32x32;

  // Create a new 32x32 NIOSII packet
  let mut packet = PktType::new(1, 2, 3, 4);

  // Print debug output of a newly created packet
  println!("{:#?}", packet);

  // Print display output of a newly created packet
  println!("{}", packet);

  // Get pointer to underlying buffer
  let _ptr = packet.as_mut_ptr();

  // Convert a packet into a vector (underlying buffer is reused)
  let packet_vec = packet.into_vec();

  // Convert a vector back into a packet
  let mut reused_packet = PktType::reuse(packet_vec);

  // Check if a valid packet has been created:
  reused_packet.validate().expect("Failed to validate");

  // Get individual field of a packet
  let _target_id = reused_packet.target_id();

  // Set individual field of a packet
  let _target_id = reused_packet.set_target_id(0x33);

  // Check if packet indicates success
  let _success = reused_packet.is_success();

  // Check if a packet defines write or read operation
  let _success = reused_packet.is_write();

  Ok(())
}
```

## Design Goals

- NIOSII Packet should be able to be serialized and deserialized
- There should be one default implementation valid for all kinds of NIOSII packet types. Reimplementing for each Packet type is not an option.
- Existing buffers should be reusable (allow for zero alloc)
