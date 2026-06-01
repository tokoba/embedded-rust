/* STM32F767ZI (2MB Flash / 512KB RAM) 用 memory.x
 *
 * Program領域(FLASH) と User領域(USER_FLASH) を分けて、
 * リンカがUser領域を使用しないようにする。
 */

MEMORY
{
  /* Total: 2048KB(2MB) */
  /* Program 領域: 0x0800_0000 .. 0x081B_FFFF (1792KB)
   * Program Region: Sector 0 - Sector 10               */
  FLASH (rx)      : ORIGIN = 0x08000000, LENGTH = 1792K

  /* User 領域(予約): 0x081C_0000 .. 0x081F_FFFF (256KB)
   * 256K (Sector 11)                                   */
  USER_FLASH (rx) : ORIGIN = 0x081C0000, LENGTH = 256K

  /* RAM: 512KB (DTCM + SRAM1 + SRAM2)                  */
  RAM (rwx)       : ORIGIN = 0x20000000, LENGTH = 512K
}

/* Rust 側から参照するためのシンボル（必要なら使う） */
__stack_start = ORIGIN(RAM) + LENGTH(RAM);
__user_flash_start = ORIGIN(USER_FLASH);
__user_flash_end   = ORIGIN(USER_FLASH) + LENGTH(USER_FLASH);
