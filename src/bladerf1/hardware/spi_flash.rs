use crate::bladerf1::nios_client::NiosClient;
use crate::error::{Error, Result};
use crate::flash::{
    BLADERF_FLASH_PAGE_SIZE, BLADERF_FLASH_TOTAL_PAGES, BLADERF_FLASH_TOTAL_SECTORS,
};
use crate::transport::usb::{
    BLADE_USB_CMD_FLASH_ERASE, BLADE_USB_CMD_FLASH_READ, BLADE_USB_CMD_FLASH_WRITE,
    BLADE_USB_CMD_READ_CAL_CACHE, BLADE_USB_CMD_READ_PAGE_BUFFER, BLADE_USB_CMD_WRITE_PAGE_BUFFER,
    USB_IF_CONFIG, USB_IF_RF_LINK, USB_IF_SPI_FLASH, UsbInterfaceCommands,
};
use nusb::Speed;

pub fn get_chunk_size(speed: Speed) -> Result<usize> {
    match speed {
        Speed::Super | Speed::SuperPlus => Ok(BLADERF_FLASH_PAGE_SIZE),
        Speed::High => Ok(64),
        _ => Err(Error::UnsupportedSpeed),
    }
}

pub fn with_flash_mode<T>(
    nios: &mut NiosClient,
    f: impl FnOnce(&mut NiosClient) -> Result<T>,
) -> Result<T> {
    nios.usb_change_setting(USB_IF_SPI_FLASH)?;
    let result = f(nios);
    let restore_setting = match nios.nios_config_read() {
        Ok(gpio) if gpio != 0 => USB_IF_RF_LINK,
        _ => USB_IF_CONFIG,
    };
    if let Err(e) = nios.usb_change_setting(restore_setting) {
        log::warn!("Failed to restore USB alt setting after flash mode: {e:#}");
    }
    result
}

fn flash_cmd(nios: &mut NiosClient, cmd: u8, w_index: u16) -> Result<()> {
    let status = nios.usb_vendor_cmd_int_w_index(cmd, w_index)?;
    if status != 0 {
        Err(Error::FlashError(status))
    } else {
        Ok(())
    }
}

fn write_chunks(
    nios: &mut NiosClient,
    chunk_size: usize,
    buf: &[u8; BLADERF_FLASH_PAGE_SIZE],
) -> Result<()> {
    for offset in (0..BLADERF_FLASH_PAGE_SIZE).step_by(chunk_size) {
        let end = (offset + chunk_size).min(BLADERF_FLASH_PAGE_SIZE);
        nios.usb_vendor_cmd_out_w_index(
            BLADE_USB_CMD_WRITE_PAGE_BUFFER,
            offset as u16,
            &buf[offset..end],
        )?;
    }
    Ok(())
}

fn read_chunks(
    nios: &mut NiosClient,
    chunk_size: usize,
    cmd: u8,
    buf: &mut [u8; BLADERF_FLASH_PAGE_SIZE],
) -> Result<()> {
    for offset in (0..BLADERF_FLASH_PAGE_SIZE).step_by(chunk_size) {
        let end = (offset + chunk_size).min(BLADERF_FLASH_PAGE_SIZE);
        nios.usb_vendor_cmd_in_w_index_data(cmd, offset as u16, &mut buf[offset..end])?;
    }
    Ok(())
}

pub fn erase_sector(nios: &mut NiosClient, _chunk_size: usize, sector: u16) -> Result<()> {
    if sector >= BLADERF_FLASH_TOTAL_SECTORS {
        return Err(Error::Argument(format!(
            "flash sector {sector} out of range (0..{BLADERF_FLASH_TOTAL_SECTORS})"
        )));
    }
    with_flash_mode(nios, |nios| {
        flash_cmd(nios, BLADE_USB_CMD_FLASH_ERASE, sector)
    })
}

pub fn read_page(
    nios: &mut NiosClient,
    chunk_size: usize,
    page: u16,
    buf: &mut [u8; BLADERF_FLASH_PAGE_SIZE],
) -> Result<()> {
    if page >= BLADERF_FLASH_TOTAL_PAGES {
        return Err(Error::Argument(format!(
            "flash page {page} out of range (0..{BLADERF_FLASH_TOTAL_PAGES})"
        )));
    }
    with_flash_mode(nios, |nios| {
        flash_cmd(nios, BLADE_USB_CMD_FLASH_READ, page)?;
        read_chunks(nios, chunk_size, BLADE_USB_CMD_READ_PAGE_BUFFER, buf)
    })
}

pub fn write_page(
    nios: &mut NiosClient,
    chunk_size: usize,
    page: u16,
    buf: &[u8; BLADERF_FLASH_PAGE_SIZE],
) -> Result<()> {
    if page >= BLADERF_FLASH_TOTAL_PAGES {
        return Err(Error::Argument(format!(
            "flash page {page} out of range (0..{BLADERF_FLASH_TOTAL_PAGES})"
        )));
    }
    with_flash_mode(nios, |nios| {
        write_chunks(nios, chunk_size, buf)?;
        flash_cmd(nios, BLADE_USB_CMD_FLASH_WRITE, page)
    })
}

pub fn erase_and_write_page(
    nios: &mut NiosClient,
    chunk_size: usize,
    sector: u16,
    page: u16,
    buf: &[u8; BLADERF_FLASH_PAGE_SIZE],
) -> Result<()> {
    if sector >= BLADERF_FLASH_TOTAL_SECTORS {
        return Err(Error::Argument(format!(
            "flash sector {sector} out of range (0..{BLADERF_FLASH_TOTAL_SECTORS})"
        )));
    }
    if page >= BLADERF_FLASH_TOTAL_PAGES {
        return Err(Error::Argument(format!(
            "flash page {page} out of range (0..{BLADERF_FLASH_TOTAL_PAGES})"
        )));
    }
    with_flash_mode(nios, |nios| {
        flash_cmd(nios, BLADE_USB_CMD_FLASH_ERASE, sector)?;
        write_chunks(nios, chunk_size, buf)?;
        flash_cmd(nios, BLADE_USB_CMD_FLASH_WRITE, page)
    })
}

pub fn read_cal_cache(
    nios: &mut NiosClient,
    chunk_size: usize,
    buf: &mut [u8; BLADERF_FLASH_PAGE_SIZE],
) -> Result<()> {
    with_flash_mode(nios, |nios| {
        read_chunks(nios, chunk_size, BLADE_USB_CMD_READ_CAL_CACHE, buf)
    })
}
