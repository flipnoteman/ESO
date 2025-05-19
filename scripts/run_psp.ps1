# Set Environment Variables
$RUN = "eso.prx"
$TARGET_DIR = "\target\mipsel-sony-psp\release\"
cargo-psp build --release

#start "" "usbhostfs_pc.exe"
$usbhost_process = Start-Process usbhostfs_pc.exe -ArgumentList $TARGET_DIR -PassThru
Write-Output "usbhostfs_pc.exe started at $TARGET_DIR."
Start-Sleep -Milliseconds 800
pspsh -e $RUN
Write-Output "$RUN running..."
Wait-Process -Id $usbhost_process.Id
Write-Output "usbhostfs_pc.exe stopped. Exiting..."
Start-Sleep -Milliseconds 800
pspsh -e "reset"
Write-Output "Psplink successfully restarted"

