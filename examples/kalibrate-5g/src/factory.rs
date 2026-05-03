pub(crate) fn factory_dac_trim(serial: &str) -> Option<u16> {
    let url = format!("https://www.nuand.com/calibration/?serial={serial}");

    let response = match minreq::get(&url).send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Factory calibration lookup failed: {e}");
            return None;
        }
    };
    if response.status_code != 200 {
        eprintln!(
            "Factory calibration lookup returned status {}",
            response.status_code
        );
        return None;
    }

    let body = match response.as_str() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Factory calibration body decode failed: {e}");
            return None;
        }
    };

    let pattern = "Hex: 0x";
    match body.find(pattern) {
        Some(mut start) => {
            start += pattern.len();
            let factory_dac_trim = &body[start..start + 4];
            match u16::from_str_radix(factory_dac_trim, 16) {
                Ok(val) => Some(val),
                Err(_) => {
                    eprintln!("Parsing DAC value: \"{factory_dac_trim}\" failed");
                    None
                }
            }
        }
        None => {
            eprintln!("Pattern: \"{pattern}\" not found");
            None
        }
    }
}
