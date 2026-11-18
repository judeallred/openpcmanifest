A **snapshot** is the raw data OpenPCManifest read from your PC, saved as a file. Comparing two snapshots shows exactly what changed: a new drive, a BIOS update, a different GPU driver, a RAM swap.

## In the app

1. **File → Save snapshot** before you change anything.
2. Make your upgrade, then open OpenPCManifest again.
3. **File → Compare with snapshot** and pick the file from step 1.

You'll see a **Changes** list: *Added*, *Removed* and *Changed* (old → new). Values that change on their own, such as temperatures, free space and uptime, are ignored. **← Back to full report** returns to normal.

**File → Open snapshot** shows a saved snapshot as a full report, which is handy for looking at another PC's specs.

## From the command line

```powershell
openpcmanifest --save-raw before.json
# ...upgrade...
openpcmanifest --compare before.json
openpcmanifest --compare monday.json friday.json --markdown
```

## Privacy

Snapshots contain everything that was read, including serial numbers and network addresses. Keep them private; share redacted reports instead.
