use crate::bladerf1::GainDb;
use crate::hardware::lms6002d::{
    BLADERF_LNA_GAIN_MAX, BLADERF_RXVGA1_GAIN_MAX, BLADERF_RXVGA1_GAIN_MIN,
    BLADERF_RXVGA2_GAIN_MAX, BLADERF_RXVGA2_GAIN_MIN, LMS6002D, LnaGainCode,
};
use crate::{Error, Result};
use std::cmp::PartialEq;

/// This structure is used to directly apply DC calibration register values to
/// the LMS, rather than use the values resulting from an auto-calibration.
///
/// A value < 0 is used to denote that the specified value should not be written.
/// If a value is to be written, it will be truncated to 8-bits.
#[derive(Debug)]
pub struct DcCals {
    /// LPF tuning module
    lpf_tuning: i16,
    /// TX LPF I filter
    tx_lpf_i: i16,
    /// TX LPF Q filter
    tx_lpf_q: i16,
    /// RX LPF I filter
    rx_lpf_i: i16,
    /// RX LPF Q filter
    rx_lpf_q: i16,
    /// RX VGA2 DC reference module
    dc_ref: i16,
    /// RX VGA2, I channel of first gain stage
    rxvga2a_i: i16,
    /// RX VGA2, Q channel of first gain stage
    rxvga2a_q: i16,
    /// RX VGA2, I channel of second gain stage
    rxvga2b_i: i16,
    /// RX VGA2, Q channel of second gain stage
    rxvga2b_q: i16,
}

pub struct DcCalState {
    /// Backup of clock enables
    clk_en: u8,
    /// Register backup
    reg0x72: u8,
    ///  Backup of gain values
    lna_gain: LnaGainCode,
    rxvga1_gain: i32,
    rxvga2_gain: i32,

    /// Base address of DC cal regs
    base_addr: u8,
    /// # of DC cal submodules to operate on
    num_submodules: u32,
    /// Current gains used in retry loops
    rxvga1_curr_gain: i32,
    rxvga2_curr_gain: i32,
}

/// DC Calibration Modules
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum DcCalModule {
    Invalid = -1,
    LpfTuning,
    TxLpf,
    RxLpf,
    RxVga2,
}

impl LMS6002D {
    /// Reference LMS6002D calibration guide, section 4.1 flow chart
    pub fn dc_cal_loop(&self, base: u8, cal_address: u8, dc_cntval: u8) -> Result<u8> {
        // %2.2x:%2.2x
        log::debug!("Calibrating module {base:#x}:{cal_address:#x}");

        // Set the calibration address for the block and start it up
        let mut val = self.read(base + 0x03)?;

        val &= !0x07;
        val |= cal_address & 0x07;

        self.write(base + 0x03, val)?;

        // Set and latch the DC_CNTVAL
        self.write(base + 0x02, dc_cntval)?;

        val |= 1 << 4;
        self.write(base + 0x03, val)?;

        val &= !(1 << 4);
        self.write(base + 0x03, val)?;

        // Start the calibration by toggling DC_START_CLBR
        val |= 1 << 5;
        self.write(base + 0x03, val)?;

        val &= !(1 << 5);
        self.write(base + 0x03, val)?;

        // Main loop checking the calibration
        for _ in 0..25 {
            // Read active low DC_CLBR_DONE
            val = self.read(base + 0x01)?;

            // Check if calibration is done
            if ((val >> 1) & 1) == 0 {
                // Per LMS FAQ item 4.7, we should check
                // DC_REG_VAL, as DC_LOCK is not a reliable indicator
                let dc_regval = self.read(base)? & 0x3f;
                log::debug!("DC_REGVAL: {dc_regval}");
                return Ok(dc_regval);
            }
        }

        log::warn!("DC calibration loop did not converge.");
        // status = BLADERF_ERR_UNEXPECTED;
        Err(Error::Invalid)
    }

    pub fn dc_cal_backup(&self, module: DcCalModule) -> Result<DcCalState> {
        let mut state = DcCalState {
            clk_en: self.read(0x09)?,
            reg0x72: 0,
            lna_gain: LnaGainCode::BypassLna1Lna2,
            rxvga1_gain: 0,
            rxvga2_gain: 0,
            base_addr: 0,
            num_submodules: 0,
            rxvga1_curr_gain: 0,
            rxvga2_curr_gain: 0,
        };

        if module == DcCalModule::RxLpf || module == DcCalModule::RxVga2 {
            state.reg0x72 = self.read(0x72)?;
            state.lna_gain = LnaGainCode::from(self.lna_get_gain()?);
            state.rxvga1_gain = self.rxvga1_get_gain()?.db as i32;
            state.rxvga2_gain = self.rxvga2_get_gain()?.db as i32;
        }

        Ok(state)
    }

    pub fn dc_cal_module_init(&self, module: DcCalModule) -> Result<DcCalState> {
        let mut state = DcCalState {
            clk_en: 0,
            reg0x72: 0,
            lna_gain: LnaGainCode::BypassLna1Lna2,
            rxvga1_gain: 0,
            rxvga2_gain: 0,
            base_addr: 0,
            num_submodules: 0,
            rxvga1_curr_gain: 0,
            rxvga2_curr_gain: 0,
        };
        let cal_clock: u8;
        let val: u8;

        match module {
            DcCalModule::LpfTuning => {
                // CLK_EN[5] - LPF CAL Clock
                cal_clock = 1 << 5;
                state.base_addr = 0x00;
                state.num_submodules = 1;
            }
            DcCalModule::TxLpf => {
                // CLK_EN[1] - TX LPF DCCAL Clock
                cal_clock = 1 << 1;
                state.base_addr = 0x30;
                state.num_submodules = 2;
            }
            DcCalModule::RxLpf => {
                // CLK_EN[3] - RX LPF DCCAL Clock
                cal_clock = 1 << 3;
                state.base_addr = 0x50;
                state.num_submodules = 2;
            }
            DcCalModule::RxVga2 => {
                // CLK_EN[4] - RX VGA2 DCCAL Clock
                cal_clock = 1 << 4;
                state.base_addr = 0x60;
                state.num_submodules = 5;
            }
            _ => return Err(Error::Invalid),
        }

        // Enable the appropriate clock based on the module
        self.write(0x09, state.clk_en | cal_clock)?;

        match module {
            DcCalModule::LpfTuning => {
                // Nothing special to do
            }
            DcCalModule::RxLpf | DcCalModule::RxVga2 => {
                // FAQ 5.26 (rev 1.0r10) notes that the DC comparators should be
                // powered up when performing DC calibration and then powered down
                // afterward to improve receiver linearity.
                if module == DcCalModule::RxVga2 {
                    self.clear(0x6e, 3 << 6)?;
                } else {
                    // Power up RX LPF DC calibration comparator
                    self.clear(0x5f, 1 << 7)?;
                }

                // Disconnect LNA from the RXMIX input by opening up the
                // INLOAD_LNA_RXFE switch. This should help reduce external
                // interference while calibrating
                val = state.reg0x72 & !(1 << 7);
                self.write(0x72, val)?;

                // Attempt to calibrate at max gain.
                self.lna_set_gain(GainDb {
                    db: BLADERF_LNA_GAIN_MAX,
                })?;

                state.rxvga1_curr_gain = BLADERF_RXVGA1_GAIN_MAX as i32;
                self.rxvga1_set_gain(GainDb {
                    db: state.rxvga1_curr_gain as i8,
                })?;

                state.rxvga2_curr_gain = BLADERF_RXVGA2_GAIN_MAX as i32;
                self.rxvga2_set_gain(GainDb {
                    db: state.rxvga2_curr_gain as i8,
                })?;
            }
            DcCalModule::TxLpf => {
                // FAQ item 4.1 notes that the DAC should be turned off or set
                // to generate minimum DC
                self.set(0x36, 1 << 7)?;

                // Ensure TX LPF DC calibration comparator is powered up
                self.clear(0x3f, 1 << 7)?;
            }
            _ => {
                // assert(!"Invalid module");
                return Err(Error::Invalid);
            }
        }

        Ok(state)
    }

    /// The RXVGA2 items here are based upon Lime Microsystems' recommendations
    /// in their "Improving RxVGA2 DC Offset Calibration Stability" Document:
    /// https://groups.google.com/group/limemicro-opensource/attach/19b675d099a22b89/Improving%20RxVGA2%20DC%20Offset%20Calibration%20Stability_v1.pdf?part=0.1&authuser=0
    ///
    ///  This function assumes that the submodules are preformed in a consecutive
    ///  and increasing order, as outlined in the above document.
    pub fn dc_cal_submodule(
        &self,
        module: DcCalModule,
        submodule: u8,
        state: &DcCalState,
    ) -> Result<bool> {
        let mut converged: bool = false;

        if module == DcCalModule::RxVga2 {
            match submodule {
                0 => {
                    // Reset VGA2GAINA and VGA2GAINB to the default power-on values,
                    // in case we're retrying this calibration due to one of the
                    // later submodules failing. For the same reason, RXVGA2 decode
                    // is disabled; it is not used for the RC reference module (0)

                    // Disable RXVGA2 DECODE
                    self.clear(0x64, 1 << 0)?;

                    // VGA2GAINA = 0, VGA2GAINB = 0
                    self.write(0x68, 0x01)?;
                }
                1 => {
                    // Setup for Stage 1 I and Q channels (submodules 1 and 2)

                    // Set to direct control signals: RXVGA2 Decode = 1
                    self.set(0x64, 1 << 0)?;

                    // VGA2GAINA = 0110, VGA2GAINB = 0
                    self.write(0x68, 0x06)?;
                }
                2 => {
                    // No additional changes needed - covered by previous execution
                    // of submodule == 1.
                }
                3 => {
                    // Setup for Stage 2 I and Q channels (submodules 3 and 4)

                    // VGA2GAINA = 0, VGA2GAINB = 0110
                    self.write(0x68, 0x60)?;
                }
                4 => {
                    // No additional changes needed - covered by execution
                    // of submodule == 3
                }
                _ => {
                    //assert(!"Invalid submodule");
                    return Err(Error::Invalid);
                }
            }
        }

        let mut dc_regval = self.dc_cal_loop(state.base_addr, submodule, 31)?;

        if dc_regval == 31 {
            log::debug!("DC_REGVAL suboptimal value - retrying DC cal loop.");

            // FAQ item 4.7 indcates that can retry with DC_CNTVAL reset
            dc_regval = self.dc_cal_loop(state.base_addr, submodule, 0)?;
            if dc_regval == 0 {
                log::debug!("Bad DC_REGVAL detected. DC cal failed.");
                return Ok(converged);
            }
        }

        if module == DcCalModule::LpfTuning {
            // Special case for LPF tuning module where results are
            // written to TX/RX LPF DCCAL

            // Set the DC level to RX and TX DCCAL modules
            let mut val = self.read(0x35)?;
            val &= !0x3f;
            val |= dc_regval;
            self.write(0x35, val)?;

            val = self.read(0x55)?;
            val &= !0x3f;
            val |= dc_regval;
            self.write(0x55, val)?;
        }

        converged = true;
        Ok(converged)
    }

    pub fn dc_cal_retry_adjustment(
        &self,
        module: DcCalModule,
        state: &mut DcCalState,
    ) -> Result<bool> {
        let mut limit_reached: bool = false;

        match module {
            DcCalModule::LpfTuning | DcCalModule::TxLpf => {
                // Nothing to adjust here
                limit_reached = true;
            }
            DcCalModule::RxLpf => {
                if state.rxvga1_curr_gain > BLADERF_RXVGA1_GAIN_MIN as i32 {
                    state.rxvga1_curr_gain -= 1;
                    log::debug!("Retrying DC cal with RXVGA1={}", state.rxvga1_curr_gain);
                    self.rxvga1_set_gain(GainDb {
                        db: state.rxvga1_curr_gain as i8,
                    })?;
                } else {
                    limit_reached = true;
                }
            }
            DcCalModule::RxVga2 => {
                if state.rxvga1_curr_gain > BLADERF_RXVGA1_GAIN_MIN as i32 {
                    state.rxvga1_curr_gain -= 1;
                    log::debug!("Retrying DC cal with RXVGA1={}", state.rxvga1_curr_gain);
                    self.rxvga1_set_gain(GainDb {
                        db: state.rxvga1_curr_gain as i8,
                    })?;
                } else if state.rxvga2_curr_gain > BLADERF_RXVGA2_GAIN_MIN as i32 {
                    state.rxvga2_curr_gain -= 3;
                    log::debug!("Retrying DC cal with RXVGA2={}", state.rxvga2_curr_gain);
                    self.rxvga2_set_gain(GainDb {
                        db: state.rxvga2_curr_gain as i8,
                    })?;
                } else {
                    limit_reached = true;
                }
            }
            _ => {
                // limit_reached = true;
                // assert(!"Invalid module");
                // status = BLADERF_ERR_UNEXPECTED;
                return Err(Error::Invalid);
            }
        }

        if limit_reached {
            log::debug!("DC Cal retry limit reached");
        }
        Ok(limit_reached)
    }

    pub fn dc_cal_module_deinit(&self, module: DcCalModule) -> Result<()> {
        match module {
            DcCalModule::LpfTuning => {
                // Nothing special to do here
            }
            DcCalModule::RxLpf => {
                // Power down RX LPF calibration comparator
                self.set(0x5f, 1 << 7)?;
            }
            DcCalModule::RxVga2 => {
                // Restore defaults: VGA2GAINA = 1, VGA2GAINB = 0
                self.write(0x68, 0x01)?;

                // Disable decode control signals: RXVGA2 Decode = 0
                self.clear(0x64, 1 << 0)?;

                // Power DC comparitors down, per FAQ 5.26 (rev 1.0r10)
                self.set(0x6e, 3 << 6)?;
            }
            DcCalModule::TxLpf => {
                // Power down TX LPF DC calibration comparator
                self.set(0x3f, 1 << 7)?;

                // Re-enable the DACs
                self.clear(0x36, 1 << 7)?;
            }
            _ => {
                // assert(!"Invalid module");
                // status = BLADERF_ERR_INVAL;
                return Err(Error::Invalid);
            }
        }

        Ok(())
    }

    pub fn dc_cal_restore(&self, module: DcCalModule, state: &DcCalState) -> Result<()> {
        self.write(0x09, state.clk_en)?;

        if module == DcCalModule::RxLpf || module == DcCalModule::RxVga2 {
            self.write(0x72, state.reg0x72)?;
            self.lna_set_gain(state.lna_gain.into())?;
            self.rxvga1_set_gain(GainDb {
                db: state.rxvga1_gain as i8,
            })?;
            self.rxvga2_set_gain(GainDb {
                db: state.rxvga2_gain as i8,
            })?;
        }
        Ok(())
    }

    pub fn dc_cal_module(&self, module: DcCalModule, state: &mut DcCalState) -> Result<bool> {
        let mut converged = true;

        for submodule in 0..state.num_submodules as u8 {
            converged = self.dc_cal_submodule(module, submodule, state)?;
            if !converged {
                return Err(Error::Invalid);
            }
        }

        Ok(converged)
    }

    pub fn lms_calibrate_dc(&self, module: DcCalModule) -> Result<()> {
        let oldstate = self.dc_cal_backup(module)?;

        let state = self.dc_cal_module_init(module);
        // if (status != 0) {
        //     goto error;
        // }
        if state.is_err() {
            let _ = self.dc_cal_module_deinit(module);
            let _ = self.dc_cal_restore(module, &oldstate);
            return Err(Error::Invalid);
        }

        let mut ok_state = state.unwrap();

        let mut converged = false;
        let mut limit_reached = false;

        while !converged && !limit_reached {
            converged = self.dc_cal_module(module, &mut ok_state)?;

            if !converged {
                limit_reached = self.dc_cal_retry_adjustment(module, &mut ok_state)?;
            }
        }

        if !converged {
            log::warn!("DC Calibration (module={module:?}) failed to converge.");
            //status = BLADERF_ERR_UNEXPECTED;
            return Err(Error::Invalid);
        }

        Ok(())

        // error:
        //     tmp_status = dc_cal_module_deinit(dev, module, &state);
        //     status = (status != 0) ? status : tmp_status;
        //
        //     tmp_status = dc_cal_restore(dev, module, &state);
        //     status = (status != 0) ? status : tmp_status;
        //
        //     return status;
    }

    pub fn set_cal_clock(&self, enable: bool, mask: u8) -> Result<()> {
        if enable {
            self.set(0x09, mask)
        } else {
            self.clear(0x09, mask)
        }
    }

    pub fn enable_lpf_cal_clock(&self, enable: bool) -> Result<()> {
        self.set_cal_clock(enable, 1 << 5)
    }

    // TODO:
    /// Enables or disables the RXVGA2 DC calibration clock.
    ///
    /// This function controls the RXVGA2 DC calibration clock by setting or clearing
    /// the corresponding bit (bit 4) in the register at address `0x09`.
    ///
    /// # Parameters
    /// - `enable`: A boolean indicating whether
    pub fn enable_rxvga2_dccal_clock(&self, enable: bool) -> Result<()> {
        self.set_cal_clock(enable, 1 << 4)
    }

    pub fn enable_rxlpf_dccal_clock(&self, enable: bool) -> Result<()> {
        self.set_cal_clock(enable, 1 << 3)
    }

    pub fn enable_txlpf_dccal_clock(&self, enable: bool) -> crate::Result<()> {
        self.set_cal_clock(enable, 1 << 1)
    }

    pub fn set_dc_cal_value(&self, base: u8, dc_addr: u8, value: u8) -> crate::Result<u8> {
        let mut regval: u8 = 0x08 | dc_addr;

        // Keep reset inactive, cal disable, load addr
        self.write(base + 3, regval)?;

        // Update DC_CNTVAL
        self.write(base + 2, value)?;

        // Strobe DC_LOAD
        regval |= 1 << 4;
        self.write(base + 3, regval)?;

        regval &= !(1 << 4);
        self.write(base + 3, regval)?;

        self.read(base)
    }

    pub fn get_dc_cal_value(&self, base: u8, dc_addr: u8) -> crate::Result<u8> {
        // Keep reset inactive, cal disable, load addr
        self.write(base + 3, 0x08 | dc_addr)?;

        // Fetch value from DC_REGVAL
        self.read(base)
    }

    /// Manually load values into LMS6002 DC calibration registers.
    ///
    /// This is generally intended for applying a set of known values resulting from
    /// a previous run of the LMS autocalibrations.
    ///
    /// @param       dev        Device handle
    /// @param[in]   dc_cals    Calibration values to load. Values set to <0 will
    ///                          not be applied.
    ///
    /// @return 0 on success, value from \ref RETCODES list on failure
    pub fn set_dc_cals(&self, dc_cals: DcCals) -> crate::Result<()> {
        let cal_tx_lpf: bool = (dc_cals.tx_lpf_i >= 0) || (dc_cals.tx_lpf_q >= 0);

        let cal_rx_lpf: bool = (dc_cals.rx_lpf_i >= 0) || (dc_cals.rx_lpf_q >= 0);

        let cal_rxvga2: bool = (dc_cals.dc_ref >= 0)
            || (dc_cals.rxvga2a_i >= 0)
            || (dc_cals.rxvga2a_q >= 0)
            || (dc_cals.rxvga2b_i >= 0)
            || (dc_cals.rxvga2b_q >= 0);

        if dc_cals.lpf_tuning >= 0 {
            self.enable_lpf_cal_clock(true)?;
            self.set_dc_cal_value(0x00, 0, dc_cals.lpf_tuning as u8)?;
            self.enable_lpf_cal_clock(false)?;
        }

        if cal_tx_lpf {
            self.enable_txlpf_dccal_clock(true)?;

            if dc_cals.tx_lpf_i >= 0 {
                self.set_dc_cal_value(0x30, 0, dc_cals.tx_lpf_i as u8)?;
            }

            if dc_cals.tx_lpf_q >= 0 {
                self.set_dc_cal_value(0x30, 1, dc_cals.tx_lpf_q as u8)?;
            }

            self.enable_txlpf_dccal_clock(false)?;
        }

        if cal_rx_lpf {
            self.enable_rxlpf_dccal_clock(true)?;

            if dc_cals.rx_lpf_i >= 0 {
                self.set_dc_cal_value(0x50, 0, dc_cals.rx_lpf_i as u8)?;
            }

            if dc_cals.rx_lpf_q >= 0 {
                self.set_dc_cal_value(0x50, 1, dc_cals.rx_lpf_q as u8)?;
            }

            self.enable_rxlpf_dccal_clock(false)?;
        }

        if cal_rxvga2 {
            self.enable_rxvga2_dccal_clock(true)?;

            if dc_cals.dc_ref >= 0 {
                self.set_dc_cal_value(0x60, 0, dc_cals.dc_ref as u8)?;
            }

            if dc_cals.rxvga2a_i >= 0 {
                self.set_dc_cal_value(0x60, 1, dc_cals.rxvga2a_i as u8)?;
            }

            if dc_cals.rxvga2a_q >= 0 {
                self.set_dc_cal_value(0x60, 2, dc_cals.rxvga2a_q as u8)?;
            }

            if dc_cals.rxvga2b_i >= 0 {
                self.set_dc_cal_value(0x60, 3, dc_cals.rxvga2b_i as u8)?;
            }

            if dc_cals.rxvga2b_q >= 0 {
                self.set_dc_cal_value(0x60, 4, dc_cals.rxvga2b_q as u8)?;
            }

            self.enable_rxvga2_dccal_clock(false)?;
        }

        Ok(())
    }

    pub fn get_dc_cals(&self) -> crate::Result<DcCals> {
        Ok(DcCals {
            lpf_tuning: self.get_dc_cal_value(0x00, 0)? as i16,
            tx_lpf_i: self.get_dc_cal_value(0x30, 0)? as i16,
            tx_lpf_q: self.get_dc_cal_value(0x30, 1)? as i16,
            rx_lpf_i: self.get_dc_cal_value(0x50, 0)? as i16,
            rx_lpf_q: self.get_dc_cal_value(0x50, 1)? as i16,
            dc_ref: self.get_dc_cal_value(0x60, 0)? as i16,
            rxvga2a_i: self.get_dc_cal_value(0x60, 1)? as i16,
            rxvga2a_q: self.get_dc_cal_value(0x60, 2)? as i16,
            rxvga2b_i: self.get_dc_cal_value(0x60, 3)? as i16,
            rxvga2b_q: self.get_dc_cal_value(0x60, 4)? as i16,
        })
    }
}
