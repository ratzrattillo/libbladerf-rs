libbladerf-rs is a pure rust implementation for interacting with a Nuand BladeRF1.


To view the documentation, build it with:
```bash
cargo doc --open
```

Examples on how to use libbladerf-rs can be found in the `examples/` directory

# Testing

## Run tests using the following command

```bash
cargo test -- --test-threads=1
```

## Run tests and display output

```bash
cargo test -- --nocapture --test-threads=1
```

## Run a specific test and display output

```bash
cargo test --test bladerf1_tuning -- --nocapture --test-threads=1
```

# LibBladeRF-RS
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

## Design Goals

- NIOSII Packet should be able to be serialized and deserialized
- There should be one default implementation valid for all kinds of NIOSII packet types. Reimplementing for each Packet type is not an option.
- Existing buffers should be reusable (allow for zero alloc)
