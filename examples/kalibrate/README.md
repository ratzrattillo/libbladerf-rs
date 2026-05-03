# kalibrate

Calibrate the bladeRF's VCTCXO oscillator (38.4 MHz) using GSM FCCH signals as a frequency reference.

## Usage

```
cargo run -p kalibrate -- [OPTIONS]

Options:
  -C, --dac-trim <VALUE>   DAC trim value (decimal or 0x hex)
  -w, --write              Write DAC trim to flash
```

### Examples

Auto-calibrate from current DAC value:
```
cargo run -p kalibrate
```

Auto-calibrate with a specific starting DAC value:
```
cargo run -p kalibrate -- -C 0x8000
```

Write a known DAC value to flash (skip calibration):
```
cargo run -p kalibrate -- -w -C 0x802E
```

## How it works

The tool performs these steps automatically:
1. Scans all GSM bands for the strongest signal
2. Tunes to that frequency
3. Binary-searches the DAC trim value to minimize frequency offset
4. Refines with a fine linear search when close
5. Prints the final offset, frequency, PPM, and DAC value

Requirements: bladeRF x40 or x115, GSM basestation signal in range.

## Output

```
[*] Current DAC Trim: Some(0x802e)
[*] Factory DAC Trim: Some(0x8030)
[*] Scanning all bands for strongest signal...
[*] Strongest signal: 945.000 MHz  power: 15.23  Calibrating...
Calibrating at 945.000 MHz...
DAC: 32768 (0x8000)  offset: +182.5kHz
...
offset: -12 Hz  freq: 944.999988 MHz  ppm: -0.01  DAC: 32814 (0x802E)
```

When writing to flash:
```
DAC trim 32814 (0x802E) written to flash.
```

## Debug output

Set `RUST_LOG=debug` for verbose diagnostic output:
```
RUST_LOG=debug cargo run -p kalibrate
```
