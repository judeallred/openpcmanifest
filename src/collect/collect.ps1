# OpenPCManifest collector.
#
# Runs under the inbox Windows PowerShell 5.1 and writes one UTF-8 JSON
# document to $OutFile. Every query is read-only. Each query runs in its own
# try/catch so one failure (for example, a class that needs administrator
# rights) never blanks the whole report; failures are listed under "errors".
#
# Contributors: you can paste any block below into a PowerShell window to see
# exactly what it returns on your machine.

param([Parameter(Mandatory = $true)][string]$OutFile)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$out = [ordered]@{}
$errs = New-Object System.Collections.ArrayList

function Iso($d) {
    if ($null -eq $d) { return $null }
    try { return ([datetime]$d).ToUniversalTime().ToString('o') } catch { return $null }
}

function B64($bytes) {
    if ($null -eq $bytes) { return $null }
    return [Convert]::ToBase64String([byte[]]$bytes)
}

function Str($chars) {
    if ($null -eq $chars) { return $null }
    return (-join ($chars | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ })).Trim()
}

# Runs a query and stores its results as an array under $name.
function Q([string]$name, [scriptblock]$block) {
    try { $out[$name] = @(& $block) }
    catch { [void]$errs.Add([ordered]@{ source = $name; message = $_.Exception.Message }) }
}

# Runs a query and stores a single value under $name.
function One([string]$name, [scriptblock]$block) {
    try { $out[$name] = (& $block) }
    catch { [void]$errs.Add([ordered]@{ source = $name; message = $_.Exception.Message }) }
}

function RegValue([string]$path, [string]$name) {
    try { return (Get-ItemProperty -Path $path -Name $name -ErrorAction Stop).$name } catch { return $null }
}

$out['schema'] = 1
$out['collectedAt'] = (Get-Date).ToUniversalTime().ToString('o')
One 'isAdmin' {
    ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
}
$out['psVersion'] = $PSVersionTable.PSVersion.ToString()

# ---- Operating system -------------------------------------------------------
Q 'os' {
    Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber, OSArchitecture,
        SerialNumber, RegisteredUser, CSName, Locale, ProductType, TotalVisibleMemorySize,
        FreePhysicalMemory, TotalVirtualMemorySize, FreeVirtualMemory,
        @{ n = 'InstallDate'; e = { Iso $_.InstallDate } }, @{ n = 'LastBootUpTime'; e = { Iso $_.LastBootUpTime } }
}
One 'windowsVersion' {
    $cv = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    [ordered]@{
        displayVersion = RegValue $cv 'DisplayVersion'
        ubr = RegValue $cv 'UBR'
        editionId = RegValue $cv 'EditionID'
        productName = RegValue $cv 'ProductName'
    }
}
Q 'computerSystem' {
    Get-CimInstance Win32_ComputerSystem | Select-Object Manufacturer, Model, SystemType, Domain, Workgroup,
        PartOfDomain, TotalPhysicalMemory, UserName, PCSystemType, HypervisorPresent, Name
}
One 'timeZone' { (Get-TimeZone).DisplayName }
One 'culture' { (Get-Culture).Name }
One 'firmwareType' { $env:firmware_type }
One 'uac' {
    $p = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
    [ordered]@{ enableLua = RegValue $p 'EnableLUA'; consentPromptBehaviorAdmin = RegValue $p 'ConsentPromptBehaviorAdmin' }
}
Q 'antivirus' {
    Get-CimInstance -Namespace root\SecurityCenter2 -ClassName AntiVirusProduct |
        Select-Object displayName, productState
}
Q 'firewall' { Get-NetFirewallProfile | Select-Object Name, @{ n = 'Enabled'; e = { "$($_.Enabled)" } } }
One 'secureBoot' {
    [ordered]@{
        enabled = RegValue 'HKLM:\SYSTEM\CurrentControlSet\Control\SecureBoot\State' 'UEFISecureBootEnabled'
        ca2023Status = RegValue 'HKLM:\SYSTEM\CurrentControlSet\Control\SecureBoot\Servicing' 'UEFICA2023Status'
    }
}
Q 'tpm' {
    Get-CimInstance -Namespace root\cimv2\Security\MicrosoftTpm -ClassName Win32_Tpm |
        Select-Object IsEnabled_InitialValue, IsActivated_InitialValue, SpecVersion, ManufacturerIdTxt,
            ManufacturerVersion
}
Q 'hotfixes' {
    Get-CimInstance Win32_QuickFixEngineering | Select-Object HotFixID, Description,
        @{ n = 'InstalledOn'; e = { Iso $_.InstalledOn } }
}
One 'runtimes' {
    $ndp = 'HKLM:\SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full'
    $pwsh = Get-Command pwsh -ErrorAction SilentlyContinue
    $java = Get-Command java -ErrorAction SilentlyContinue
    [ordered]@{
        dotnetFramework = RegValue $ndp 'Version'
        windowsPowerShell = $PSVersionTable.PSVersion.ToString()
        powershell7 = if ($pwsh) { $pwsh.Version.ToString() } else { $null }
        java = if ($java) { $java.Version.ToString() } else { $null }
    }
}
One 'powerPlan' { (powercfg /getactivescheme | Out-String).Trim() }

# ---- Processor, memory, board -----------------------------------------------
Q 'processor' {
    Get-CimInstance Win32_Processor | Select-Object Name, Manufacturer, NumberOfCores,
        NumberOfLogicalProcessors, MaxClockSpeed, SocketDesignation, L2CacheSize, L3CacheSize,
        VirtualizationFirmwareEnabled, SecondLevelAddressTranslationExtensions, Revision
}
Q 'physicalMemory' {
    Get-CimInstance Win32_PhysicalMemory | Select-Object BankLabel, DeviceLocator, Capacity, Speed,
        ConfiguredClockSpeed, Manufacturer, PartNumber, SerialNumber, SMBIOSMemoryType, FormFactor,
        ConfiguredVoltage
}
Q 'memoryArray' {
    Get-CimInstance Win32_PhysicalMemoryArray | Select-Object MemoryDevices, MaxCapacity, MaxCapacityEx, Use
}
One 'smbios' {
    $t = Get-CimInstance -Namespace root\wmi -ClassName MSSmBios_RawSMBiosTables | Select-Object -First 1
    [ordered]@{
        version = "$($t.SmbiosMajorVersion).$($t.SmbiosMinorVersion)"
        data = B64 $t.SMBiosData
    }
}
Q 'baseBoard' { Get-CimInstance Win32_BaseBoard | Select-Object Manufacturer, Product, Version, SerialNumber }
Q 'bios' {
    Get-CimInstance Win32_BIOS | Select-Object Manufacturer, SMBIOSBIOSVersion, SerialNumber,
        @{ n = 'ReleaseDate'; e = { Iso $_.ReleaseDate } }
}
Q 'systemSlots' {
    Get-CimInstance Win32_SystemSlot | Select-Object SlotDesignation, CurrentUsage, MaxDataWidth
}

# ---- Graphics and displays --------------------------------------------------
Q 'videoControllers' {
    Get-CimInstance Win32_VideoController | Select-Object Name, AdapterCompatibility, DriverVersion,
        AdapterRAM, VideoProcessor, CurrentHorizontalResolution, CurrentVerticalResolution,
        CurrentRefreshRate, CurrentBitsPerPixel, PNPDeviceID, Status,
        @{ n = 'DriverDate'; e = { Iso $_.DriverDate } }
}
Q 'gpuMemory' {
    $class = 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}'
    Get-ChildItem $class -ErrorAction SilentlyContinue | Where-Object { $_.PSChildName -match '^\d{4}$' } |
        ForEach-Object {
            $p = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue
            [ordered]@{
                driverDesc = $p.DriverDesc
                matchingDeviceId = $p.MatchingDeviceId
                memorySize = $p.'HardwareInformation.qwMemorySize'
            }
        }
}
Q 'monitors' {
    Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID | ForEach-Object {
        [ordered]@{
            instanceName = $_.InstanceName
            manufacturer = Str $_.ManufacturerName
            name = Str $_.UserFriendlyName
            productCode = Str $_.ProductCodeID
            serial = Str $_.SerialNumberID
            week = $_.WeekOfManufacture
            year = $_.YearOfManufacture
        }
    }
}
Q 'monitorParams' {
    Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorBasicDisplayParams |
        Select-Object InstanceName, MaxHorizontalImageSize, MaxVerticalImageSize, VideoInputType
}

# ---- Storage ----------------------------------------------------------------
Q 'diskDrives' {
    Get-CimInstance Win32_DiskDrive | Select-Object Index, Model, SerialNumber, FirmwareRevision,
        InterfaceType, MediaType, Size, PNPDeviceID, Partitions, Status
}
Q 'physicalDisks' {
    Get-PhysicalDisk | ForEach-Object {
        [ordered]@{
            deviceId = "$($_.DeviceId)"
            friendlyName = $_.FriendlyName
            mediaType = "$($_.MediaType)"
            busType = "$($_.BusType)"
            size = $_.Size
            healthStatus = "$($_.HealthStatus)"
            spindleSpeed = $_.SpindleSpeed
            firmwareVersion = $_.FirmwareVersion
        }
    }
}
Q 'reliability' {
    Get-PhysicalDisk | ForEach-Object {
        $id = "$($_.DeviceId)"
        $r = $_ | Get-StorageReliabilityCounter
        [ordered]@{
            deviceId = $id
            temperature = $r.Temperature
            temperatureMax = $r.TemperatureMax
            wear = $r.Wear
            powerOnHours = $r.PowerOnHours
            readErrorsUncorrected = $r.ReadErrorsUncorrected
            writeErrorsUncorrected = $r.WriteErrorsUncorrected
            startStopCycleCount = $r.StartStopCycleCount
        }
    }
}
Q 'smartData' {
    Get-CimInstance -Namespace root\wmi -ClassName MSStorageDriver_FailurePredictData | ForEach-Object {
        [ordered]@{ instanceName = $_.InstanceName; data = B64 $_.VendorSpecific }
    }
}
Q 'smartThresholds' {
    Get-CimInstance -Namespace root\wmi -ClassName MSStorageDriver_FailurePredictThresholds | ForEach-Object {
        [ordered]@{ instanceName = $_.InstanceName; data = B64 $_.VendorSpecific }
    }
}
Q 'smartStatus' {
    Get-CimInstance -Namespace root\wmi -ClassName MSStorageDriver_FailurePredictStatus |
        Select-Object InstanceName, PredictFailure
}
Q 'partitions' {
    Get-Partition | ForEach-Object {
        [ordered]@{
            diskNumber = $_.DiskNumber
            partitionNumber = $_.PartitionNumber
            driveLetter = if ($_.DriveLetter -and $_.DriveLetter -ne [char]0) { "$($_.DriveLetter)" } else { $null }
            size = $_.Size
            type = "$($_.Type)"
        }
    }
}
Q 'logicalDisks' {
    Get-CimInstance Win32_LogicalDisk | Select-Object DeviceID, FileSystem, Size, FreeSpace, VolumeName,
        VolumeSerialNumber, DriveType
}
Q 'cdrom' {
    Get-CimInstance Win32_CDROMDrive | Select-Object Name, Drive, MediaLoaded, MediaType, Status,
        CapabilityDescriptions, SCSIBus, SCSILogicalUnit, SCSIPort, SCSITargetId
}

# ---- Audio, peripherals, USB, devices ---------------------------------------
Q 'soundDevices' { Get-CimInstance Win32_SoundDevice | Select-Object Name, Manufacturer, Status }
Q 'audioEndpoints' {
    Get-PnpDevice -Class AudioEndpoint -PresentOnly | Select-Object FriendlyName, Status
}
Q 'keyboards' { Get-CimInstance Win32_Keyboard | Select-Object Name, Description, PNPDeviceID }
Q 'pointingDevices' {
    Get-CimInstance Win32_PointingDevice | Select-Object Name, Manufacturer, PNPDeviceID
}
Q 'printers' {
    Get-CimInstance Win32_Printer | Select-Object Name, PortName, Default, DriverName, Network, Shared,
        WorkOffline
}
Q 'usbDevices' {
    Get-CimInstance Win32_PnPEntity -Filter "PNPDeviceID LIKE 'USB\\%'" |
        Select-Object Name, PNPClass, Manufacturer, PNPDeviceID, Status
}
Q 'usbControllers' { Get-CimInstance Win32_USBController | Select-Object Name, Manufacturer }
Q 'deviceProblems' {
    Get-CimInstance Win32_PnPEntity -Filter 'ConfigManagerErrorCode <> 0' |
        Select-Object Name, PNPClass, ConfigManagerErrorCode, PNPDeviceID, Present
}
Q 'drivers' {
    Get-CimInstance Win32_PnPSignedDriver -Filter "DeviceName IS NOT NULL" | Select-Object DeviceName,
        DeviceClass, DriverVersion, DriverProviderName, IsSigned, @{ n = 'DriverDate'; e = { Iso $_.DriverDate } }
}

# ---- Network ----------------------------------------------------------------
Q 'netAdapters' {
    Get-NetAdapter | Select-Object Name, InterfaceDescription, Status, LinkSpeed, MacAddress, MediaType,
        PhysicalMediaType, ifIndex, Virtual, DriverVersion
}
Q 'ipConfig' {
    Get-CimInstance Win32_NetworkAdapterConfiguration -Filter 'IPEnabled = TRUE' | Select-Object Description,
        InterfaceIndex, IPAddress, IPSubnet, DefaultIPGateway, DNSServerSearchOrder, DHCPEnabled, DHCPServer
}
Q 'shares' { Get-CimInstance Win32_Share | Select-Object Name, Path, Type }
One 'remoteDesktop' {
    [ordered]@{ denyConnections = RegValue 'HKLM:\System\CurrentControlSet\Control\Terminal Server' 'fDenyTSConnections' }
}
One 'proxy' {
    $p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
    [ordered]@{ enabled = RegValue $p 'ProxyEnable'; server = RegValue $p 'ProxyServer'; autoConfigUrl = RegValue $p 'AutoConfigURL' }
}
One 'wlanInterfaces' { (netsh wlan show interfaces | Out-String) }

$out['errors'] = @($errs)
$json = $out | ConvertTo-Json -Depth 6 -Compress
[IO.File]::WriteAllText($OutFile, $json, (New-Object System.Text.UTF8Encoding($false)))
