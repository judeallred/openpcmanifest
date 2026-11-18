## It takes a few seconds to load

Reading hardware goes through Windows' own inventory services, and some queries (installed drivers especially) take a few seconds on any machine. OpenPCManifest runs them in parallel and shows each section as soon as its data arrives, so the Summary, CPU and RAM appear first. The line at the top lists what's still loading.

## Windows SmartScreen blocks it

Builds aren't signed until v1.0. Choose **More info → Run anyway**. See [[Getting Started]].

## Drive health says "Unknown"

Run as administrator. See [[Drive Health]].

## Windows 11 check says TPM is "Unknown"

TPM details need administrator rights. Run as administrator and **Refresh**.

## A section is missing or a value is blank

OpenPCManifest leaves out anything Windows doesn't report rather than guessing, and virtual machines in particular report very little. **About This Report** lists any queries that failed and why.

## "Could not read this PC"

The app couldn't start Windows PowerShell. It needs `C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe`, which every supported Windows has. Some locked-down corporate PCs block it; ask your IT team, or run `openpcmanifest --text` from a terminal to see the full error.

## Something is wrong

[Open an issue](https://github.com/judeallred/openpcmanifest/issues/new/choose) with a redacted report:

```powershell
openpcmanifest --text --redact --out report.txt
```

Attach `report.txt` and say what you expected to see.
