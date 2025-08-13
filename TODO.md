- in basic.rs config_gpio_write: Speed info should not be determined on every call of gpio_write, but rather at global board_data level.
- maybe have one hardcoded pre-reserved Vector that is reused by every call to nios_send.
- Get rid of experimental_control_urb method.
- Clarify, when to use asserts and when not
- Can dependencies between crates be shared, so they are not compiled twice into the different crates?
- Ranges for frequency and sample rate are currently continuous, but this is not supported by the Hardware.
- NIOS packet don't claim endpoint on every call to nios_send. This is very slow, as acquiring and releasing an endpoint takes more time,
  than acquiring it once for the whole lifetime of the BladeRf1 session and accessing it via Mutex
- Code Clarity could be increased with custom types for GainDb and GainCode. Because different parts of the code consume tha Gain in different Formats
  This might help to avoid confusion between the "From"-Traits impl for BladeRfLnaGain and functions like _convert_gain_to_lna_gain
- How to doc comment parameters in Rust?
- Adjusts the quadrature DC offset. Valid values are \[-2048, 2048\], whichare scaled to the available control bits. DcoffQ, that's why they maybe just go until 2016?
- Fix BladeRf being in weird state, where recieving e.g. via fm-receiver example, after running the tests, no proper output is produced.. (White noise only)
- impl From<&LmsFreq> for u64 { maybe impl deref for lmsfreq

NIOS:
- Assert and trow error, if in retune packet the maximum width of nint and nfrac is reached
- Instead of asserts, we could use normal throwing of errors. This has the benefit
  of allowing the application to decide how to handle such an error
- Test boundries of fields in the tests e.g. limit of bits is not exceeded.
- Fully implement checks in packet validation() methods