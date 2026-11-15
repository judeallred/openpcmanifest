# OpenPCManifest user guide

> These pages seed the project's GitHub wiki, which the app's **Help → User guide** opens. To
> publish them, enable the wiki in the repository settings and copy each file in `docs/wiki/` to a
> wiki page with the same name.

OpenPCManifest shows what's inside your Windows PC and answers upgrade and health questions. It is
free, open source, portable and never connects to the internet.

## Getting started

1. Download `openpcmanifest.exe` from the
   [latest release](https://github.com/judeallred/openpcmanifest/releases/latest).
2. Double-click it. If Windows SmartScreen appears, choose **More info → Run anyway** (builds are
   unsigned until v1.0).
3. Wait a few seconds while it reads your hardware.

For drive health details and TPM information, right-click the exe and choose **Run as
administrator**.

## Common questions

**Can I add more RAM?** Open **Upgrade Advisor → Memory**. It shows used and free slots, the most
memory your board supports, and what type to buy.

**Why does my RAM run slower than the box says?** If **Upgrade Advisor** shows "Free speed", your
memory is running at its fallback speed. Turning on the XMP (Intel) or EXPO (AMD) profile in your
BIOS usually fixes it. Check your board manual first.

**Is my drive dying?** Open **Storage**. Each drive shows Good, Caution or Bad with the reason.
*Caution* means back up now and keep an eye on it; *Bad* means replace it soon. Health needs
administrator rights on most systems.

**Why can't it see my empty M.2 slot?** Windows doesn't report empty M.2 sockets or SATA ports.
Your board manual lists them.

**How do I share my specs on a forum?** Click **Copy summary** and choose Markdown (Reddit,
GitHub, Discord) or BBCode (most forums). Private details are hidden automatically.

**What does "Hide private details" hide?** Serial numbers, MAC and IP addresses, product IDs,
Wi-Fi network names, computer and user names, printer ports and volume serials.

**Does it show temperatures over time?** No. OpenPCManifest is an inventory tool. For live
monitoring, use LibreHardwareMonitor or HWiNFO.

## Reporting a problem

Use **Help → Report a bug**. Include the output of
`openpcmanifest --text --redact --out report.txt` (run from a terminal in the exe's folder) so we
can see what your PC reported.
