**Is it really free? What's the catch?**
No catch. It's MIT-licensed open source: no ads, no account, no telemetry, no bundled software, and no paid tier.

**Does it send my data anywhere?**
No. There's no networking code in the program. The only thing that can reach the internet is your browser, when you click a Help link.

**Is it safe to run as administrator?**
It only reads system information and changes nothing. Every query it runs is in one readable script, [collect.ps1](https://github.com/judeallred/openpcmanifest/blob/main/src/collect/collect.ps1).

**How is it different from Speccy?**
It shows the same hardware inventory, plus an [[Upgrade Advisor]], plain-language [[Drive Health]], device problems, driver age, a Windows 11 check, search, redaction and [[Snapshots and Compare]]. It's open source and installs nothing.

**Does it show temperatures over time?**
No. It's an inventory tool and shows a temperature only where a drive reports one. For live monitoring use [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) or [HWiNFO](https://www.hwinfo.com/).

**Why can't it see my empty M.2 slot?**
Windows doesn't report empty M.2 sockets or SATA ports. Your board manual lists them.

**Why does my RAM run slower than the box says?**
See **Free speed** in [[Upgrade Advisor]].

**Does it work on 32-bit Windows?**
Yes: download the `windows-x86` build.

**Mac or Linux?**
Not yet. On other systems the command-line tool can open and compare snapshots taken on Windows.

**When are winget and Chocolatey coming?**
With v1.0, along with signed builds.

**How can I help?**
Run it and [add your PC to the testing grid](https://github.com/judeallred/openpcmanifest/blob/main/docs/TESTED_SYSTEMS.md), report bugs, or see [CONTRIBUTING.md](https://github.com/judeallred/openpcmanifest/blob/main/CONTRIBUTING.md).
