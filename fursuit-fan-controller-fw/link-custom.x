/* This file is fed into the linker in addition to the automatically generated linker scripts
by embassy-stm32 (memory.x), defmt (defmt.x) and cortex-m-rt (link.x). */

SECTIONS
{
  .flash_filesystem /*(NOLOAD)*/ : /* uncomment the (NOLOAD) to not have "cargo run" overwrite the filesystem section with zeroes */
  ALIGN(1024) /* Flash is organized in 1 KiB pages */
  {
    KEEP(*(.flash_filesystem))
  } > FLASH
}
