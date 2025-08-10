- Inconsistent naming of Bladerf like e.g. BladerfRationalRate or BladeRfRationalRate
- in basic.rs config_gpio_write: Speed info should not be determined on every call of gpio_write, but rather at global board_data level.
- maybe have one hardcoded pre-reserved Vector that is reused by every call to nios_send.
- Get rid of experimental_control_urb method.
- Is a separate crate for nios and global variables really required??
- Clarify, when to use asserts and when not
- Adjust log-levels according to https://stackoverflow.com/questions/76753965/when-to-choose-the-trace-log-level-over-the-debug-log-level
- Can dependencies between crates be shared, so they are not compiled twice into the different crates?
- Ranges for frequency and sample rate are currently continuous, but this is not supported by the Hardware.
- NIOS packet don't claim endpoint on every call to nios_send. This is very slow, as acquiring and releasing an endpoint takes more time,
  than acquiring it once for the whole lifetime of the BladeRf1 session and accessing it via Mutex
- Code Clarity could be increased with custom types for GainDb and GainCode. Because different parts of the code consume tha Gain in different Formats
  This might help to avoid confusion between the "From"-Traits impl for BladeRfLnaGain and functions like _convert_gain_to_lna_gain
- How to doc comment parameters in Rust?
- Search for \n in all files to stop having weird linebreaks
- Adjusts the quadrature DC offset. Valid values are \[-2048, 2048\], which
  are scaled to the available control bits.
  DcoffQ, that's why they maybe just go until 2016?
- Fix Error: Nusb(Error { kind: Busy, code: None, message: "endpoint already in use" }) when running tests...
  Problem: When claiming an endpoint on creation of the BladeRf, the endpoint cannot be found, as the bladerf has not yet been initialized.
  Also, the endpoint becomes eventually unusable, if the underlying interface settings are changed...
- Fix BladeRf being in weird state, where recieving e.g. via fm-receiver example, after running the tests, no proper output is produced.. (White noise only)
- Fix locking the interface several times in the same method. One method should not acquire a lock to an interface several times...
