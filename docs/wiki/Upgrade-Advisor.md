The **Upgrade Advisor** section answers "what can I add to this PC?" from what the hardware reports.

## Memory

- **Installed**: how much RAM you have, and the most your board supports (when the board reports it).
- **Slots**: how many are used and free.
- **You can**: what to buy. With free slots, it tells you how many modules of which type (for example DDR4) you can add, and to match the existing speed so every module runs at full speed. With every slot full, it tells you the largest total you can reach by swapping in bigger modules.
- **Free speed** (amber): your memory runs slower than it's rated for. This is very common. Memory starts at a safe default speed until you turn on its rated profile in the BIOS, called **XMP** on Intel boards and **EXPO** (or DOCP) on AMD boards. Check your board manual for where the setting is. It's usually one switch.

## Expansion slots

Each free slot is listed with its type and lane width, for example *PCIe 4.0, x4 lanes*. Some boards share lanes between slots and M.2 sockets, so filling one can disable another; the board manual has the details.

## Storage

Lists your drives by type (NVMe SSD, SATA SSD, HDD). If any drive shows warning signs, **Replace first** suggests dealing with that before spending money on anything else.

## What Windows can't tell us

Windows doesn't report **empty M.2 sockets** or **empty SATA ports**, so the advisor can't list them. Search for your board's manual. The advisor shows the exact search to use.
