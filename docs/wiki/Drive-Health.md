# Drive Health

Each drive in **Storage** gets a verdict with the reason in plain words.

| Verdict | Means | Do this |
| --- | --- | --- |
| **Good** | No warning signs. | Nothing. Keep your usual backups. |
| **Caution** | Early warning signs, such as reallocated or pending sectors, or most of an SSD's rated endurance used. | **Back up now.** Keep an eye on it; plan to replace it. |
| **Bad** | An attribute has crossed its failure threshold, or the drive predicts its own failure. | **Back up immediately** and replace the drive. |

The reason follows the verdict, for example *Caution: 16 reallocation events*.

## Where the verdict comes from

1. The drive's **SMART attributes** and their thresholds (needs administrator rights on most PCs).
2. Windows' own **health status** for the drive.
3. **Reliability counters**: uncorrected read and write errors, and SSD wear.

The worst of these wins. The full SMART table is under **SMART attributes** for each drive if you want the raw numbers.

## "Unknown (run as administrator…)"

SMART data needs administrator rights. Right-click the exe, choose **Run as administrator**, then **Refresh**.

## "Unknown (the drive does not report SMART data)"

Some drives don't expose SMART to Windows, notably drives behind many RAID controllers, some USB enclosures, and virtual disks. Windows' health status is still used when available.

## A note on caution

A Caution verdict doesn't mean a drive will fail soon; many drives run for years with a handful of reallocated sectors. It means the drive has started using spare space to work around damage, which is the moment to make sure your backups are current.
