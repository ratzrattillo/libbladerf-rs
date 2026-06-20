//! Trigger control for coordinated RX/TX streaming.
//!
//! Provides master/slave trigger synchronization so multiple channels can
//! start streaming simultaneously. The master channel arms and fires the
//! trigger; slave channels arm and wait for the master's fire signal.

use crate::bladerf1::board::RfLinkSession;
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::protocol::nios::NiosPkt8x8Target;

/// Role of a channel in the trigger synchronization scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerRole {
    /// Master channel: can arm and fire the trigger to synchronize peers.
    Master,
    /// Slave channel: arms and waits for the master to fire the trigger.
    Slave,
}

/// Current trigger state for a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerState {
    /// Active role if the trigger is armed; `None` if disarmed.
    role: Option<TriggerRole>,
    /// `true` if the trigger has been fired (asserted).
    fired: bool,
    /// `true` if a fire command has been issued by the master.
    fire_requested: bool,
}
impl TriggerState {
    pub fn new(role: Option<TriggerRole>, fired: bool, fire_requested: bool) -> Self {
        Self {
            role,
            fired,
            fire_requested,
        }
    }

    pub fn role(&self) -> Option<TriggerRole> {
        self.role
    }

    pub fn fired(&self) -> bool {
        self.fired
    }

    pub fn fire_requested(&self) -> bool {
        self.fire_requested
    }
}

const REG_ARM: u8 = 1 << 0;
const REG_FIRE: u8 = 1 << 1;
const REG_MASTER: u8 = 1 << 2;
const REG_LINE: u8 = 1 << 3;

fn trigger_target(channel: Channel) -> NiosPkt8x8Target {
    match channel {
        Channel::Tx => NiosPkt8x8Target::TxTriggerCtl,
        Channel::Rx => NiosPkt8x8Target::RxTriggerCtl,
    }
}

impl RfLinkSession<'_> {
    fn trigger_read(&mut self, channel: Channel) -> Result<u8> {
        self.nios.nios_read::<u8, u8>(trigger_target(channel), 0)
    }

    fn trigger_write(&mut self, channel: Channel, value: u8) -> Result<()> {
        self.nios
            .nios_write::<u8, u8>(trigger_target(channel), 0, value)
    }

    /// Arms the trigger for a channel with the given role.
    ///
    /// Sets the trigger to the armed state and assigns the channel as master
    /// or slave. Clears the fire bit. Only the master can subsequently fire
    /// the trigger via `fire_trigger`.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn arm_trigger(&mut self, channel: Channel, role: TriggerRole) -> Result<()> {
        self.require_initialized()?;
        let reg = self.trigger_read(channel)?;
        let new_reg = (reg & !(REG_FIRE | REG_MASTER))
            | REG_ARM
            | match role {
                TriggerRole::Master => REG_MASTER,
                TriggerRole::Slave => 0,
            };
        self.trigger_write(channel, new_reg)
    }

    /// Fires the trigger on the master channel to synchronize armed peers.
    ///
    /// Only the master channel may fire the trigger. Returns an error if the
    /// trigger is not armed or if the channel is not configured as master.
    ///
    /// Returns `Error::BoardState` if the board is not initialized, the
    /// trigger is not armed, or the channel is not the master.
    pub fn fire_trigger(&mut self, channel: Channel) -> Result<()> {
        self.require_initialized()?;
        let reg = self.trigger_read(channel)?;
        if (reg & REG_ARM) == 0 {
            return Err(Error::BoardState("trigger not armed"));
        }
        if (reg & REG_MASTER) == 0 {
            return Err(Error::BoardState("only master can fire trigger"));
        }
        self.trigger_write(channel, reg | REG_FIRE)
    }

    /// Disarms the trigger for a channel, clearing all trigger state.
    ///
    /// Resets arm, fire, and master flags. The channel returns to the default
    /// untriggered streaming mode.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn disarm_trigger(&mut self, channel: Channel) -> Result<()> {
        self.require_initialized()?;
        let reg = self.trigger_read(channel)?;
        self.trigger_write(channel, reg & !(REG_ARM | REG_FIRE | REG_MASTER))
    }

    /// Returns the current trigger state for a channel.
    ///
    /// Indicates whether the trigger is armed (and with which role), whether
    /// the trigger line has fired, and whether a fire has been requested.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn trigger_state(&mut self, channel: Channel) -> Result<TriggerState> {
        self.require_initialized()?;
        let reg = self.trigger_read(channel)?;
        let role = if (reg & REG_ARM) != 0 {
            Some(if (reg & REG_MASTER) != 0 {
                TriggerRole::Master
            } else {
                TriggerRole::Slave
            })
        } else {
            None
        };
        Ok(TriggerState::new(
            role,
            (reg & REG_LINE) == 0,
            (reg & REG_FIRE) != 0,
        ))
    }
}
