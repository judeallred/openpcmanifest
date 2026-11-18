# Getting Started

## Download

1. Open the [latest release](https://github.com/judeallred/openpcmanifest/releases/latest), or the [nightly build](https://github.com/judeallred/openpcmanifest/releases/tag/nightly) for the newest changes.
2. Pick your version:
   - **`windows-x64`**: 64-bit Windows. This is almost every PC made since 2010.
   - **`windows-x86`**: 32-bit Windows, for older machines. Not sure? Open **Settings → System → About** and look at **System type**.
3. Download the `.exe` (or the `.zip` if you'd like the README and licence alongside it).

Nothing is installed. You can run it from your Downloads folder or a USB stick.

## Supported systems

Windows 10 version 1809 or later, Windows 11, and Windows Server 2019, 2022 and 2025. It uses Windows PowerShell 5.1, which is built into all of them.

## First run

Double-click `openpcmanifest.exe`.

**If Windows SmartScreen says it "protected your PC":** builds aren't code-signed yet (that's planned for v1.0). Click **More info**, then **Run anyway**. If you'd like to check the file first, compare its checksum with the one in `SHA256SUMS.txt` on the release page:

```powershell
Get-FileHash .\openpcmanifest-windows-x64.exe -Algorithm SHA256
```

The window opens straight away and fills in section by section as your hardware is read. The **Summary**, **CPU**, **RAM** and **Motherboard** appear first; drives, network, devices and drivers follow over the next few seconds. A line at the top says what's still loading.

## Run as administrator for the full picture

Some details need administrator rights: drive SMART data (used for [[Drive Health]]), drive reliability counters, and TPM details for the Windows 11 check. Right-click the exe and choose **Run as administrator** to include them. Without it, the report says what's missing.

## Next

- [[Reading Your Report]]
- [[Sharing Your Specs]]
