// pub struct UsbDeviceSignature {
//     pub vid: u16,
//     pub pid: u16,
//     #[allow(dead_code)]
//     pub description: &'static str,
// }
// pub const KNOWN_DEVICES: &[UsbDeviceSignature; 2] = &[
//     UsbDeviceSignature {
//         vid: 0x2cf0,
//         pid: 0x5246,
//         description: "BladeRF 1",
//     },
//     UsbDeviceSignature {
//         vid: 0x2cf0,
//         pid: 0x5250,
//         description: "BladeRF 2",
//     },
// ];
//
// impl PartialEq for UsbDeviceSignature {
//     fn eq(&self, other: &Self) -> bool {
//         self.vid == other.vid && self.pid == other.pid
//     }
// }
