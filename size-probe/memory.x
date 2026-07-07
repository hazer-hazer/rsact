/* Generic Cortex-M layout, big enough for any of our thumb targets. These
   binaries are never flashed — they exist only so `.text/.rodata/.bss` can be
   measured from a real linked ELF (the numbers are the regression signal), so a
   representative layout is fine (see WS0.3 / the roadmap: "no budgets"). Origins
   match the common STM32 map. */
MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 256K
  RAM   : ORIGIN = 0x20000000, LENGTH = 64K
}
