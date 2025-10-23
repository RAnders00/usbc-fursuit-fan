use core::ops::Range;

use defmt::{expect, unwrap};
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_stm32::{
    Peri,
    flash::{Blocking, FLASH_BASE, Flash},
    peripherals::FLASH,
};
use embedded_storage::nor_flash::NorFlash;
use sequential_storage::cache::NoCache;

/// How many pages of the MCU's flash should the embedded filesystem take up?
/// Note: More pages will result in longer lifetime, since the wear on the flash
/// will be spread across more pages.
/// A page is 1024 bytes in size on the STM32F103C8.
#[cfg(debug_assertions)]
pub const FILESYSTEM_SIZE_PAGES: usize = 22;

// In release mode, less space is occupied by code, thus the filesystem can be larger
#[cfg(not(debug_assertions))]
pub const FILESYSTEM_SIZE_PAGES: usize = 29;

/// Size of `FLASH_FILESYSTEM_SECTION` in bytes.
const FLASH_FILESYSTEM_SECTION_SIZE: usize =
    <Flash as NorFlash>::ERASE_SIZE * FILESYSTEM_SIZE_PAGES;

/// Static allocation in flash that is:
/// 1. Aligned to a flash page boundary
/// 2. A whole multiple of a flash page in size
///
/// This section is used to place the filesystem inside. The linker script
/// `link-custom.x` handles the special .flash_filesystem section.
///
/// This is specified to be initialized with 0xFF - this is the value that will
/// be written when the MCU is flashed with a new firmware, resetting the
/// contents of the filesystem. We use 0xFF since this is what the filesystem
/// (sequential_storage) expects freshly cleared flash sectors to look like.
/// (Flash inherently assumes all 0xFF's after a flash page is erased)
/// 
/// If this was all 0's, the filesystem would report "Corrupted" errors.
#[unsafe(link_section = ".flash_filesystem")]
#[used]
static mut FLASH_FILESYSTEM_SECTION: [u8; FLASH_FILESYSTEM_SECTION_SIZE] =
    [0xFF; FLASH_FILESYSTEM_SECTION_SIZE];

const STATE_STORAGE_KEY: u8 = 0;

pub struct Persistence {
    flash: BlockingAsync<Flash<'static, Blocking>>,
    flash_range: Range<u32>,
}

impl Persistence {
    // Call this once during program initialization.
    pub fn new(flash_peri: Peri<'static, FLASH>) -> Self {
        let fs_start_addr = (&raw const FLASH_FILESYSTEM_SECTION) as usize;
        let fs_start_offset = unwrap!(fs_start_addr.checked_sub(FLASH_BASE));

        defmt::assert_eq!(
            size_of::<usize>(),
            size_of::<u32>(),
            "Persistence assumes usize == u32"
        );

        let flash_range =
            (fs_start_offset as u32)..((fs_start_offset + FLASH_FILESYSTEM_SECTION_SIZE) as u32);

        defmt::info!(
            "Using flash offsets 0x{:08x}-0x{:08x} for persistence",
            flash_range.start,
            flash_range.end
        );
        defmt::info!(
            "Flash is organized into {} byte pages",
            <Flash as NorFlash>::ERASE_SIZE
        );

        let flash = BlockingAsync::new(Flash::new_blocking(flash_peri));

        Self { flash, flash_range }
    }

    pub async fn save_state(&mut self, state_idx: usize) {
        let mut data_buffer = [0; 2 * <Flash as NorFlash>::ERASE_SIZE];
        let data = expect!(u8::try_from(state_idx), "state_idx did not fit inside u8");
        if let Err(e) = sequential_storage::map::store_item(
            &mut self.flash,
            self.flash_range.clone(),
            &mut NoCache::new(),
            &mut data_buffer,
            &STATE_STORAGE_KEY,
            &data,
        )
        .await
        {
            defmt::warn!("Unable to persist state to flash: {}", e);
        }
    }

    pub async fn load_state(&mut self) -> Option<usize> {
        let mut data_buffer = [0; 2 * <Flash as NorFlash>::ERASE_SIZE];
        match sequential_storage::map::fetch_item(
            &mut self.flash,
            self.flash_range.clone(),
            &mut NoCache::new(),
            &mut data_buffer,
            &STATE_STORAGE_KEY,
        )
        .await
        {
            Ok(v) => v.map(|v: u8| v as usize),
            Err(e) => {
                defmt::warn!("Unable to load state from flash: {}", e);
                None
            }
        }
    }
}
