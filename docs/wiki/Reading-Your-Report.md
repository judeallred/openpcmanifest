# Reading Your Report

Sections are listed on the left. Use the search box at the top to find anything across all of them, such as a model number, "PCIe" or "Realtek".

Coloured values mean something: **green** is good, **amber** needs attention, **red** is a problem, and *grey italics* are notes or advice.

| Section | What it tells you |
| --- | --- |
| **Summary** | One line per component. This is what **Copy summary** copies. |
| **Operating System** | Windows edition, version and build, install date, uptime, UAC, firewall and antivirus, installed runtimes, power plan, installed updates. |
| **CPU** | Model, code name and process node, cores and threads, socket, base clock, instruction sets (AVX2, AES and so on), virtualization state, cache sizes. |
| **RAM** | Slots used and free, total installed, type (DDR4, DDR5…), channels, speed, the most the board supports, and each module's maker, part number, speed and voltage. Shows an amber line if memory runs slower than it's rated for. |
| **Motherboard** | Maker, model and revision, BIOS version and age, and every expansion slot with its type, lane width and whether it's free. |
| **Graphics** | Each GPU with its real video memory, card maker, driver version and age; each monitor with its model, size and manufacture date. |
| **Storage** | Each drive's interface, capacity, firmware, health verdict, temperature, power-on hours, full SMART table, and partitions with free space. See [[Drive Health]]. |
| **Optical Drives** | Model, drive letter and capabilities. |
| **Audio** | Sound devices, and playback and recording endpoints. |
| **Peripherals** | Keyboards and mice (and how they connect), and printers. |
| **USB** | USB controllers and devices, grouped by type, with vendor and product IDs. |
| **Network** | Connections and their addresses, adapters with link speed, Wi-Fi details, Remote Desktop and proxy settings, shared folders. |
| **Upgrade Advisor** | What you can add. See [[Upgrade Advisor]]. |
| **Device Problems** | Every device Device Manager is unhappy about, with what the error code means. |
| **Drivers & Firmware Age** | Third-party drivers and the BIOS, with anything over three years old flagged. |
| **Windows 11 Readiness** | TPM, Secure Boot, UEFI, processor, memory and system drive checks. Not shown on Windows Server. |
| **About This Report** | When it was collected, administrator status, and any queries that failed. |

## Words you'll see

- **MT/s** (megatransfers per second): how memory speed is usually advertised. DDR4-3200 is 3200 MT/s, which some tools show as 1600 MHz.
- **Lanes (x1, x4, x16)**: how many PCIe lanes a slot is wired for. A graphics card wants x16; most other cards are fine in x1 or x4.
- **SMART**: the self-monitoring data drives keep about their own health.
- **[redacted]**: a private value hidden because **Hide private details** is on. See [[Sharing Your Specs]].
