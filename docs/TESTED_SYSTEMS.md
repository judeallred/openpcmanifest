# Testing grid

Real-world results from real machines. OpenPCManifest reads data that every motherboard, drive and
driver reports a little differently, so this grid is how we know what works.

## Add your PC (no coding needed)

1. Run the latest [release](https://github.com/judeallred/openpcmanifest/releases/latest) or
   [nightly](https://github.com/judeallred/openpcmanifest/releases/tag/nightly), once normally and
   once with **Run as administrator** if you can.
2. Check each section against what you know about your PC.
3. Click the pencil icon on this page on GitHub, add one row to the table below, and choose
   **Propose changes**. GitHub opens the pull request for you.

Please don't include serial numbers or anything personal. If something was wrong, also
[open an issue](https://github.com/judeallred/openpcmanifest/issues/new/choose) with a redacted
report and put the issue number in **Notes**.

**Result key:** ✅ correct · ⚠️ partly wrong or missing · ❌ wrong or crashed · – not applicable

## Results

| Date | Version | Windows | Admin | Board or laptop model | CPU | GPU | Drives | Summary | RAM / slots | Board / slots | Storage & health | Win 11 check | Notes | Tester |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |

<!-- Add your row above this line. Keep the columns in the same order. Example:
| 2026-11-20 | 0.1.0 | Windows 11 Pro 24H2 | Yes | Gigabyte B550 EAGLE WIFI6 | Ryzen 5 3600 | GTX 1070 Ti | 2 HDD, 1 SATA SSD, 1 NVMe | ✅ | ✅ | ⚠️ M.2 slot listed as PCIe x1 | ✅ | ✅ | See #12 | @yourname |
-->

Every Windows CI run also performs a live collection on a GitHub-hosted Windows Server VM. That
checks that collection runs end to end, not that the values are right for real hardware, so it is
not listed here.
